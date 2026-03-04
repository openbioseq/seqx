use std::{collections::HashSet, io::BufRead};

use clap::Parser;
use regex::Regex;

use crate::common::{Record, RecordReader, SeqWriter, detect_format};

#[derive(Parser)]
#[command(about = "Filter sequences")]
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

    /// Minimum sequence length
    #[arg(long)]
    pub min_len: Option<usize>,

    /// Maximum sequence length
    #[arg(long)]
    pub max_len: Option<usize>,

    /// Minimum GC content (%)
    #[arg(long)]
    pub gc_min: Option<f64>,

    /// Maximum GC content (%)
    #[arg(long)]
    pub gc_max: Option<f64>,

    /// Pattern to match in sequence (regex)
    #[arg(long)]
    pub pattern: Option<String>,

    /// Exclude pattern in sequence (regex)
    #[arg(long)]
    pub exclude_pattern: Option<String>,

    /// File with IDs to keep (one per line)
    #[arg(long)]
    pub id_file: Option<String>,

    /// IDs to keep (can be used multiple times)
    #[arg(long)]
    pub id: Vec<String>,

    /// Minimum average quality (FASTQ only)
    #[arg(long)]
    pub min_qual: Option<f64>,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,

    /// Invert filter (keep sequences that don't match)
    #[arg(short = 'v', long)]
    pub invert: bool,
}

fn load_ids(id_file: Option<String>, ids: Vec<String>) -> anyhow::Result<HashSet<String>> {
    let mut set: HashSet<String> = ids.into_iter().collect();

    if let Some(path) = id_file {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            let line = line?.trim().to_string();
            if !line.is_empty() {
                set.insert(line);
            }
        }
    }

    Ok(set)
}

fn check_filters(
    record: &Record,
    args: &Args,
    id_set: &HashSet<String>,
    include_regex: &Option<Regex>,
    exclude_regex: &Option<Regex>,
) -> bool {
    // 长度过滤
    if let Some(min) = args.min_len {
        if record.len() < min {
            return false;
        }
    }
    if let Some(max) = args.max_len {
        if record.len() > max {
            return false;
        }
    }

    // GC 含量过滤
    let gc = record.gc_content();
    if let Some(min) = args.gc_min {
        if gc < min {
            return false;
        }
    }
    if let Some(max) = args.gc_max {
        if gc > max {
            return false;
        }
    }

    // 模式过滤
    if let Some(regex) = include_regex {
        if !regex.is_match(&record.seq) {
            return false;
        }
    }
    if let Some(regex) = exclude_regex {
        if regex.is_match(&record.seq) {
            return false;
        }
    }

    // ID 过滤
    if !id_set.is_empty() && !id_set.contains(&record.id) {
        return false;
    }

    // 质量过滤
    if let Some(min_q) = args.min_qual {
        if let Some(avg_q) = record.avg_quality() {
            let phred_q = avg_q - 33.0;
            if phred_q < min_q {
                return false;
            }
        }
    }

    true
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;

    // 加载 ID 白名单
    let id_file = args.id_file.clone();
    let ids = args.id.clone();
    let id_set = load_ids(id_file, ids)?;

    // 编译正则表达式
    let pattern = args.pattern.clone();
    let exclude = args.exclude_pattern.clone();
    let include_regex = pattern.as_ref().map(|p| Regex::new(p)).transpose()?;
    let exclude_regex = exclude.as_ref().map(|p| Regex::new(p)).transpose()?;

    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

    let mut kept = 0;
    let mut total = 0;
    let invert = args.invert;
    let use_gc_filter = args.gc_min.is_some() || args.gc_max.is_some();

    // 流式处理
    while let Some(record) = reader.next_record()? {
        if use_gc_filter && !record.is_nucleotide_sequence() {
            return Err(anyhow::anyhow!(
                "GC filtering is only supported for nucleotide sequences. Non-nucleotide record: {}",
                record.id
            ));
        }

        total += 1;
        let mut passes = check_filters(&record, &args, &id_set, &include_regex, &exclude_regex);

        if invert {
            passes = !passes;
        }

        if passes {
            kept += 1;
            writer.write_record(&record)?;
        }
    }

    eprintln!("Filtered: {}/{} sequences kept", kept, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_filter_protein_min_len() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">p1\nMKWVTF\n>p2\nGA\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            min_len: Some(3),
            max_len: None,
            gc_min: None,
            gc_max: None,
            pattern: None,
            exclude_pattern: None,
            id_file: None,
            id: vec![],
            min_qual: None,
            line_width: 80,
            invert: false,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        assert!(out.contains(">p1"));
        assert!(!out.contains(">p2"));
    }

    #[test]
    fn test_filter_protein_gc_should_error() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">p1\nMKWVTF\n").unwrap();

        let result = run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            min_len: None,
            max_len: None,
            gc_min: Some(10.0),
            gc_max: None,
            pattern: None,
            exclude_pattern: None,
            id_file: None,
            id: vec![],
            min_qual: None,
            line_width: 80,
            invert: false,
        });

        assert!(result.is_err());
    }
}
