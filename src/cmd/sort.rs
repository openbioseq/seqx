use std::{cmp::Ordering, collections::BinaryHeap, fs::File, io::BufWriter};

use clap::Parser;
use memmap2::Mmap;
use rayon::prelude::*;
use tempfile::NamedTempFile;

use crate::common::{
    Record, RecordReader, SeqWriter, detect_format, read_record_binary_from_slice,
    write_record_binary,
};

#[derive(Parser)]
#[command(about = "Sort sequences")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Input format (auto/fasta/fastq)
    #[arg(short, long)]
    pub format: Option<String>,

    /// Sort by name
    #[arg(long, group = "sort_by")]
    pub by_name: bool,

    /// Sort by length
    #[arg(long, group = "sort_by")]
    pub by_len: bool,

    /// Sort by GC content
    #[arg(long, group = "sort_by")]
    pub by_gc: bool,

    /// Sort in descending order
    #[arg(short, long)]
    pub desc: bool,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,

    /// Maximum memory to use (MB) - falls back to external sort if exceeded
    #[arg(short = 'm', long)]
    pub max_memory: Option<usize>,

    /// Number of threads used by sort
    #[arg(short = 't', long, default_value_t = 1)]
    pub threads: usize,
}

#[derive(Clone, Copy)]
enum SortKey {
    Name,
    Length,
    Gc,
}

fn detect_sort_key(args: &Args) -> SortKey {
    if args.by_len {
        SortKey::Length
    } else if args.by_gc {
        SortKey::Gc
    } else {
        SortKey::Name
    }
}

fn compare_records(a: &Record, b: &Record, sort_key: SortKey, desc: bool) -> Ordering {
    let ord = match sort_key {
        SortKey::Name => a.id.cmp(&b.id),
        SortKey::Length => a.len().cmp(&b.len()),
        SortKey::Gc => a
            .gc_content()
            .partial_cmp(&b.gc_content())
            .unwrap_or(Ordering::Equal),
    };

    if desc { ord.reverse() } else { ord }
}

fn record_size_hint(record: &Record) -> usize {
    record.id.len()
        + record.desc.as_ref().map(|d| d.len()).unwrap_or(0)
        + record.seq.len()
        + record.qual.as_ref().map(|q| q.len()).unwrap_or(0)
        + 64
}

fn spill_sorted_chunk(
    mut records: Vec<Record>,
    sort_key: SortKey,
    desc: bool,
    temp_files: &mut Vec<NamedTempFile>,
) -> anyhow::Result<()> {
    records.par_sort_by(|a, b| compare_records(a, b, sort_key, desc));

    let tmp = NamedTempFile::new()?;
    let path = tmp.path().to_string_lossy().to_string();
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, File::create(&path)?);

    for record in records {
        write_record_binary(&mut writer, &record)?;
    }

    std::io::Write::flush(&mut writer)?;
    temp_files.push(tmp);
    Ok(())
}

#[derive(Clone)]
enum SortValue {
    Name(String),
    Length(usize),
    Gc(f64),
}

fn compare_sort_values(a: &SortValue, b: &SortValue) -> Ordering {
    match (a, b) {
        (SortValue::Name(x), SortValue::Name(y)) => x.cmp(y),
        (SortValue::Length(x), SortValue::Length(y)) => x.cmp(y),
        (SortValue::Gc(x), SortValue::Gc(y)) => x.total_cmp(y),
        _ => Ordering::Equal,
    }
}

fn sort_value_for_record(record: &Record, sort_key: SortKey) -> SortValue {
    match sort_key {
        SortKey::Name => SortValue::Name(record.id.clone()),
        SortKey::Length => SortValue::Length(record.len()),
        SortKey::Gc => SortValue::Gc(record.gc_content()),
    }
}

struct HeapItem {
    key: SortValue,
    chunk_idx: usize,
    record: Record,
    desc: bool,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.chunk_idx == other.chunk_idx && compare_sort_values(&self.key, &other.key).is_eq()
    }
}

impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut desired = compare_sort_values(&self.key, &other.key);
        if self.desc {
            desired = desired.reverse();
        }

        desired
            .reverse()
            .then_with(|| other.chunk_idx.cmp(&self.chunk_idx))
    }
}

struct MmapChunkReader {
    mmap: Mmap,
    offset: usize,
}

impl MmapChunkReader {
    fn from_file(file: &NamedTempFile) -> anyhow::Result<Self> {
        let mapped_file = File::open(file.path())?;
        let mmap = unsafe { Mmap::map(&mapped_file)? };

        Ok(Self { mmap, offset: 0 })
    }

    fn next_record(&mut self) -> anyhow::Result<Option<Record>> {
        read_record_binary_from_slice(&self.mmap, &mut self.offset)
    }
}

fn push_next_from_chunk(
    heap: &mut BinaryHeap<HeapItem>,
    chunk_idx: usize,
    chunk: &mut MmapChunkReader,
    sort_key: SortKey,
    desc: bool,
) -> anyhow::Result<()> {
    if let Some(record) = chunk.next_record()? {
        heap.push(HeapItem {
            key: sort_value_for_record(&record, sort_key),
            chunk_idx,
            record,
            desc,
        });
    }

    Ok(())
}

fn merge_chunks_to_output(
    temp_files: &[NamedTempFile],
    output: Option<&str>,
    format: crate::common::Format,
    line_width: usize,
    sort_key: SortKey,
    desc: bool,
) -> anyhow::Result<()> {
    let mut chunks: Vec<MmapChunkReader> = temp_files
        .iter()
        .map(MmapChunkReader::from_file)
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut heap = BinaryHeap::with_capacity(chunks.len());
    for (chunk_idx, chunk) in chunks.iter_mut().enumerate() {
        push_next_from_chunk(&mut heap, chunk_idx, chunk, sort_key, desc)?;
    }

    let mut writer = SeqWriter::from_path(output, format, line_width)?;

    while let Some(item) = heap.pop() {
        writer.write_record(&item.record)?;
        push_next_from_chunk(
            &mut heap,
            item.chunk_idx,
            &mut chunks[item.chunk_idx],
            sort_key,
            desc,
        )?;
    }

    writer.flush()?;
    Ok(())
}

fn estimated_chunk_count(input_bytes: usize, chunk_bytes: usize) -> usize {
    if chunk_bytes == 0 {
        1
    } else {
        input_bytes.div_ceil(chunk_bytes).max(1)
    }
}

fn choose_chunk_bytes(max_memory_mb: usize, rayon_threads: usize) -> usize {
    let base = max_memory_mb * 1024 * 1024;
    let target_parallelism = rayon_threads.max(1);
    let min_parallel_chunks = target_parallelism * 2;
    let candidate = base / min_parallel_chunks.max(1);
    candidate.max(8 * 1024 * 1024)
}

fn input_file_size(path: Option<&str>) -> Option<usize> {
    let path = path?;
    std::fs::metadata(path).ok().map(|m| m.len() as usize)
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());

    if args.threads == 0 {
        return Err(anyhow::anyhow!("--threads must be greater than 0"));
    }

    let sort_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build()?;

    let max_memory_mb = args.max_memory.unwrap_or(256).max(8);
    let rayon_threads = args.threads;
    let max_chunk_bytes = choose_chunk_bytes(max_memory_mb, rayon_threads);
    let sort_key = detect_sort_key(&args);

    let mut reader = RecordReader::new(args.input.as_deref(), format)?;
    let mut temp_files: Vec<NamedTempFile> = Vec::new();
    let mut chunk_records: Vec<Record> = Vec::new();
    let mut chunk_bytes = 0usize;

    while let Some(record) = reader.next_record()? {
        chunk_bytes += record_size_hint(&record);
        chunk_records.push(record);

        if chunk_bytes >= max_chunk_bytes {
            let records = std::mem::take(&mut chunk_records);
            sort_pool
                .install(|| spill_sorted_chunk(records, sort_key, args.desc, &mut temp_files))?;
            chunk_bytes = 0;
        }
    }

    if !chunk_records.is_empty() {
        sort_pool
            .install(|| spill_sorted_chunk(chunk_records, sort_key, args.desc, &mut temp_files))?;
    }

    if temp_files.is_empty() {
        return Ok(());
    }

    merge_chunks_to_output(
        &temp_files,
        args.output.as_deref(),
        format,
        args.line_width,
        sort_key,
        args.desc,
    )?;

    let estimated_chunks = input_file_size(args.input.as_deref())
        .map(|bytes| estimated_chunk_count(bytes, max_chunk_bytes));

    if let Some(chunks) = estimated_chunks {
        eprintln!(
            "Sorted using external mmap merge ({} chunk files, est {}, chunk target {} MB, rayon threads {})",
            temp_files.len(),
            chunks,
            max_chunk_bytes / (1024 * 1024),
            rayon_threads
        );
    } else {
        eprintln!(
            "Sorted using external mmap merge ({} chunk files, chunk target {} MB, rayon threads {})",
            temp_files.len(),
            max_chunk_bytes / (1024 * 1024),
            rayon_threads
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_sort_by_name_external() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">z\nAAAA\n>a\nTT\n>m\nCCC\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            by_name: true,
            by_len: false,
            by_gc: false,
            desc: false,
            line_width: 80,
            max_memory: Some(1),
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| &l[1..])
            .collect();
        assert_eq!(ids, vec!["a", "m", "z"]);
    }

    #[test]
    fn test_sort_by_len_desc() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">s1\nAA\n>s2\nAAAAAA\n>s3\nAAA\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            by_name: false,
            by_len: true,
            by_gc: false,
            desc: true,
            line_width: 80,
            max_memory: Some(1),
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| &l[1..])
            .collect();
        assert_eq!(ids, vec!["s2", "s3", "s1"]);
    }
}
