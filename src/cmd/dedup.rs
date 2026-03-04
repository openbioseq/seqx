use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
    fs::File,
    hash::{Hash, Hasher},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use rayon::prelude::*;
use tempfile::tempdir;

use crate::common::{
    Record, RecordReader, SeqWriter, detect_format, read_record_binary, read_u64,
    write_record_binary, write_u64,
};

#[derive(Parser)]
#[command(about = "Remove duplicate sequences")]
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

    /// Deduplicate by ID
    #[arg(long, group = "dedup_by")]
    pub by_id: bool,

    /// Deduplicate by sequence prefix (first N bases)
    #[arg(long, group = "dedup_by")]
    pub prefix: Option<usize>,

    /// Case insensitive deduplication
    #[arg(short = 'I', long)]
    pub ignore_case: bool,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,

    /// Number of disk partitions for low-memory deduplication
    #[arg(long, default_value = "128")]
    pub buckets: usize,

    /// Number of threads used by bucket dedup
    #[arg(short = 't', long, default_value_t = 1)]
    pub threads: usize,
}

#[derive(Clone)]
struct TempRecord {
    ordinal: u64,
    record: Record,
}

fn write_temp_record(writer: &mut dyn Write, temp_record: &TempRecord) -> anyhow::Result<()> {
    write_u64(writer, temp_record.ordinal)?;

    write_record_binary(writer, &temp_record.record)?;
    Ok(())
}

fn read_temp_record(reader: &mut dyn Read) -> anyhow::Result<Option<TempRecord>> {
    let Some(ordinal) = read_u64(reader)? else {
        return Ok(None);
    };

    let Some(record) = read_record_binary(reader)? else {
        return Err(anyhow::anyhow!(
            "Corrupted temp dedup record: missing payload after ordinal"
        ));
    };

    Ok(Some(TempRecord { ordinal, record }))
}

fn hash_bucket(key: &str, buckets: usize) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % buckets
}

fn bucket_path(base: &Path, prefix: &str, idx: usize) -> PathBuf {
    base.join(format!("{}_{}.bin", prefix, idx))
}

fn partition_to_disk(
    args: &Args,
    format: crate::common::Format,
    base_dir: &Path,
    buckets: usize,
) -> anyhow::Result<(usize, u64)> {
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;
    let mut writers: Vec<BufWriter<File>> = Vec::with_capacity(buckets);

    for idx in 0..buckets {
        let path = bucket_path(base_dir, "partition", idx);
        writers.push(BufWriter::new(File::create(path)?));
    }

    let mut total = 0u64;
    while let Some(record) = reader.next_record()? {
        total += 1;
        let key = record.dedup_key(args.by_id, args.prefix, args.ignore_case);
        let bucket = hash_bucket(&key, buckets);

        let temp_record = TempRecord {
            ordinal: total,
            record,
        };
        write_temp_record(&mut writers[bucket], &temp_record)?;
    }

    for writer in &mut writers {
        writer.flush()?;
    }

    Ok((buckets, total))
}

fn dedup_each_bucket(
    args: &Args,
    base_dir: &Path,
    buckets: usize,
    thread_pool: &rayon::ThreadPool,
) -> anyhow::Result<(Vec<PathBuf>, u64)> {
    let worker = || -> anyhow::Result<Vec<(usize, PathBuf, u64)>> {
        (0..buckets)
            .into_par_iter()
            .map(|idx| {
                let source = bucket_path(base_dir, "partition", idx);
                let target = bucket_path(base_dir, "unique", idx);

                let mut reader = BufReader::new(File::open(source)?);
                let mut writer = BufWriter::new(File::create(&target)?);
                let mut seen = HashSet::new();
                let mut unique_count = 0u64;

                while let Some(temp_record) = read_temp_record(&mut reader)? {
                    let key =
                        temp_record
                            .record
                            .dedup_key(args.by_id, args.prefix, args.ignore_case);

                    if seen.insert(key) {
                        write_temp_record(&mut writer, &temp_record)?;
                        unique_count += 1;
                    }
                }

                writer.flush()?;
                Ok((idx, target, unique_count))
            })
            .collect()
    };

    let mut per_bucket = thread_pool.install(worker)?;

    per_bucket.sort_by_key(|(idx, _, _)| *idx);

    let mut unique_paths = Vec::with_capacity(buckets);
    let mut unique_total = 0u64;
    for (_, path, count) in per_bucket {
        unique_paths.push(path);
        unique_total += count;
    }

    Ok((unique_paths, unique_total))
}

fn merge_unique_buckets(
    unique_paths: &[PathBuf],
    output: Option<&str>,
    format: crate::common::Format,
    line_width: usize,
) -> anyhow::Result<()> {
    let mut readers: Vec<BufReader<File>> = unique_paths
        .iter()
        .map(|p| File::open(p).map(BufReader::new))
        .collect::<Result<Vec<_>, _>>()?;

    let mut currents: Vec<Option<TempRecord>> = vec![None; readers.len()];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();

    for (idx, reader) in readers.iter_mut().enumerate() {
        currents[idx] = read_temp_record(reader)?;
        if let Some(record) = &currents[idx] {
            heap.push(Reverse((record.ordinal, idx)));
        }
    }

    let mut writer = SeqWriter::from_path(output, format, line_width)?;

    while let Some(Reverse((_, idx))) = heap.pop() {
        if let Some(temp_record) = currents[idx].take() {
            writer.write_record(&temp_record.record)?;
        }

        currents[idx] = read_temp_record(&mut readers[idx])?;
        if let Some(next) = &currents[idx] {
            heap.push(Reverse((next.ordinal, idx)));
        }
    }

    writer.flush()?;
    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let buckets = args.buckets.max(2);

    if args.threads == 0 {
        return Err(anyhow::anyhow!("--threads must be greater than 0"));
    }

    let dedup_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build()?;

    let rayon_threads = args.threads;

    let temp_dir = tempdir()?;

    let (_, total) = partition_to_disk(&args, format, temp_dir.path(), buckets)?;
    let (unique_paths, unique_total) =
        dedup_each_bucket(&args, temp_dir.path(), buckets, &dedup_pool)?;
    merge_unique_buckets(
        &unique_paths,
        args.output.as_deref(),
        format,
        args.line_width,
    )?;

    let removed = total.saturating_sub(unique_total);
    eprintln!(
        "Deduplicated: {}/{} unique sequences ({} removed, {} buckets, rayon threads {})",
        unique_total, total, removed, buckets, rayon_threads
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_dedup_by_id_preserves_first_global_order() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">a\nAAAA\n>b\nCCCC\n>a\nTTTT\n>c\nGGGG\n>b\nNNNN\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            by_id: true,
            prefix: None,
            ignore_case: false,
            line_width: 80,
            buckets: 4,
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| &l[1..])
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_dedup_by_sequence() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">r1\nAAAA\n>r2\nAAAA\n>r3\nTTTT\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            by_id: false,
            prefix: None,
            ignore_case: false,
            line_width: 80,
            buckets: 2,
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| &l[1..])
            .collect();
        assert_eq!(ids, vec!["r1", "r3"]);
    }

    #[test]
    fn test_dedup_protein_sequence() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.fa");

        fs::write(
            &input,
            ">p1\nMKWVTFISLLFLFSSAYSR\n>p2\nMKWVTFISLLFLFSSAYSR\n>p3\nGAVLIPFYWSTCMNQDEKRH\n",
        )
        .unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            by_id: false,
            prefix: None,
            ignore_case: false,
            line_width: 80,
            buckets: 8,
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let ids: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| &l[1..])
            .collect();
        assert_eq!(ids, vec!["p1", "p3"]);
    }
}
