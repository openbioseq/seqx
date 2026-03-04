use std::{
    collections::{HashMap, HashSet},
    io::{self, BufRead, Write},
};

use clap::Parser;

use crate::common::{Record, RecordReader, detect_format, parse_range};

#[derive(Parser)]
#[command(about = "Extract sequences or regions")]
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

    /// Sequence ID to extract (can be used multiple times)
    #[arg(long)]
    pub id: Vec<String>,

    /// File with IDs to extract (one per line)
    #[arg(long)]
    pub id_file: Option<String>,

    /// Range to extract (format: start:end, 1-based, inclusive)
    #[arg(long)]
    pub range: Option<String>,

    /// BED file with regions to extract
    #[arg(long)]
    pub bed: Option<String>,

    /// Flanking region size to add
    #[arg(short = 'F', long, default_value = "0")]
    pub flanking: usize,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,

    /// Stream mode: process large files by streaming (no random access)
    #[arg(long)]
    pub stream: bool,
}

#[derive(Debug, Clone)]
struct Region {
    seq_id: String,
    start: usize,
    end: usize,
    name: Option<String>,
}

fn parse_bed(path: &str) -> anyhow::Result<Vec<Region>> {
    let file = std::fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }

        let seq_id = fields[0].to_string();
        let start: usize = fields[1].parse()?;
        let end: usize = fields[2].parse()?;
        let name = fields.get(3).map(|s| s.to_string());

        regions.push(Region {
            seq_id,
            start,
            end,
            name,
        });
    }

    Ok(regions)
}

fn load_ids(id_file: Option<String>, ids: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut all_ids = ids;

    if let Some(path) = id_file {
        let file = std::fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            let line = line?.trim().to_string();
            if !line.is_empty() {
                all_ids.push(line);
            }
        }
    }

    Ok(all_ids)
}

/// 流式提取 - 按 ID 列表顺序扫描
fn extract_by_id_streaming(
    reader: &mut RecordReader,
    id_set: &HashSet<String>,
    writer: &mut dyn Write,
    line_width: usize,
) -> anyhow::Result<usize> {
    let mut found = 0;

    while let Some(record) = reader.next_record()? {
        if id_set.contains(&record.id) {
            record.write_fasta(writer, line_width)?;
            found += 1;
        }
    }

    Ok(found)
}

fn extract_record_region(
    record: &Record,
    start: usize,
    end: usize,
    flanking: usize,
    writer: &mut dyn Write,
    line_width: usize,
    name_suffix: Option<&str>,
) -> anyhow::Result<bool> {
    let actual_start = start.saturating_sub(flanking);
    let actual_end = (end + flanking).min(record.seq.len());

    if actual_start >= actual_end || actual_start >= record.seq.len() {
        return Ok(false);
    }

    let new_id = if let Some(suffix) = name_suffix {
        format!("{}_{}", record.id, suffix)
    } else {
        format!("{}_{}:{}", record.id, actual_start + 1, actual_end)
    };

    let new_record = Record::new(
        new_id,
        Some(format!(
            "extracted region {}-{}",
            actual_start + 1,
            actual_end
        )),
        record.seq[actual_start..actual_end].to_string(),
        record
            .qual
            .as_ref()
            .map(|q| q[actual_start..actual_end].to_vec()),
    );

    new_record.write_fasta(writer, line_width)?;
    Ok(true)
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());

    if format.is_fastq() {
        return Err(anyhow::anyhow!(
            "Currently only FASTA format is supported for extraction"
        ));
    }

    // 加载目标 IDs
    let ids = load_ids(args.id_file, args.id)?;
    let id_set: HashSet<String> = ids.into_iter().collect();
    let has_id_filter = !id_set.is_empty();

    // 解析 BED 区域
    let bed_regions = if let Some(bed_path) = &args.bed {
        parse_bed(bed_path)?
    } else {
        Vec::new()
    };
    let has_bed = !bed_regions.is_empty();

    let mut bed_by_seq_id: HashMap<String, Vec<Region>> = HashMap::new();
    for region in bed_regions {
        bed_by_seq_id
            .entry(region.seq_id.clone())
            .or_default()
            .push(region);
    }

    // 解析简单范围
    let simple_range = args.range.as_deref().map(parse_range).transpose()?;

    let mut writer: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(io::stdout()),
    };

    if !has_id_filter && !has_bed && simple_range.is_none() {
        return Err(anyhow::anyhow!(
            "Please specify --id, --id-file, --range, or --bed"
        ));
    }

    // 统一流式处理：单次扫描输入
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;
    let mut extracted = 0usize;

    if has_id_filter && !has_bed && simple_range.is_none() {
        extracted = extract_by_id_streaming(&mut reader, &id_set, &mut writer, args.line_width)?;
        eprintln!("Extracted: {} sequences by ID", extracted);
        return Ok(());
    }

    while let Some(record) = reader.next_record()? {
        if has_bed {
            if let Some(regions) = bed_by_seq_id.get(&record.id) {
                for region in regions {
                    let name_suffix = region.name.as_deref();
                    if extract_record_region(
                        &record,
                        region.start,
                        region.end,
                        args.flanking,
                        &mut writer,
                        args.line_width,
                        name_suffix,
                    )? {
                        extracted += 1;
                    }
                }
            }
            continue;
        }

        if let Some((start, end)) = simple_range {
            if has_id_filter && !id_set.contains(&record.id) {
                continue;
            }

            if extract_record_region(
                &record,
                start,
                end,
                args.flanking,
                &mut writer,
                args.line_width,
                None,
            )? {
                extracted += 1;
            }
        }
    }

    eprintln!("Extracted: {} sequences", extracted);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_extract_id_and_range_streaming() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">seq1\nAACCGGTT\n>seq2\nTTTTAAAA\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            id: vec!["seq1".to_string()],
            id_file: None,
            range: Some("2:5".to_string()),
            bed: None,
            flanking: 0,
            line_width: 80,
            stream: true,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        assert!(out.contains(">seq1_2:5"));
        assert!(out.contains("ACCG"));
    }

    #[test]
    fn test_extract_bed_streaming() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.fa");
        let bed = dir.path().join("r.bed");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">seq1\nAACCGGTT\n").unwrap();
        fs::write(&bed, "seq1\t1\t4\tregionA\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            id: vec![],
            id_file: None,
            range: None,
            bed: Some(bed.to_string_lossy().to_string()),
            flanking: 0,
            line_width: 80,
            stream: true,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        assert!(out.contains(">seq1_regionA"));
        assert!(out.contains("ACC"));
    }
}
