use std::{
    collections::BTreeMap,
    io::Write,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use clap::Parser;
use regex::Regex;

use crate::common::{Record, RecordReader};

#[derive(Parser)]
#[command(about = "Search patterns in sequences")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Pattern to search
    #[arg(required = true)]
    pub pattern: String,

    /// Use regex for pattern matching
    #[arg(short, long)]
    pub regex: bool,

    /// Output in BED format
    #[arg(long)]
    pub bed: bool,

    /// Report strand (for reverse complement)
    #[arg(short, long)]
    pub strand: bool,

    /// Allow mismatches (approximate matching)
    #[arg(short, long, default_value = "0")]
    pub mismatches: usize,

    /// Number of worker threads used by search
    #[arg(short = 't', long, default_value_t = 1)]
    pub threads: usize,
}

#[derive(Clone)]
struct SearchConfig {
    pattern: String,
    regex: Option<Regex>,
    bed: bool,
    strand: bool,
    mismatches: usize,
    pattern_rc: String,
    pattern_is_nucleotide: bool,
}

struct SearchJob {
    index: u64,
    record: Record,
}

struct SearchResult {
    index: u64,
    lines: Vec<String>,
}

fn reverse_complement(seq: &str) -> String {
    seq.chars()
        .rev()
        .map(|c| match c.to_ascii_uppercase() {
            'A' => 'T',
            'T' => 'A',
            'U' => 'A',
            'G' => 'C',
            'C' => 'G',
            'N' => 'N',
            c => c,
        })
        .collect()
}

fn is_nucleotide_text(seq: &str) -> bool {
    crate::common::Record::is_nucleotide_text(seq)
}

fn find_matches(
    seq: &str,
    pattern: &str,
    regex: &Option<Regex>,
    max_mismatches: usize,
) -> Vec<(usize, usize, String)> {
    let mut matches = Vec::new();
    let seq_upper = seq.to_ascii_uppercase();
    let pattern_upper = pattern.to_ascii_uppercase();

    if let Some(re) = regex {
        for mat in re.find_iter(&seq_upper) {
            matches.push((mat.start(), mat.end(), mat.as_str().to_string()));
        }
    } else if max_mismatches == 0 {
        // 精确匹配
        let pattern_bytes = pattern_upper.as_bytes();
        let seq_bytes = seq_upper.as_bytes();
        let pattern_len = pattern_bytes.len();

        for i in 0..=seq_bytes.len().saturating_sub(pattern_len) {
            if &seq_bytes[i..i + pattern_len] == pattern_bytes {
                matches.push((i, i + pattern_len, seq[i..i + pattern_len].to_string()));
            }
        }
    } else {
        // 近似匹配
        let pattern_bytes = pattern_upper.as_bytes();
        let seq_bytes = seq_upper.as_bytes();
        let pattern_len = pattern_bytes.len();

        for i in 0..=seq_bytes.len().saturating_sub(pattern_len) {
            let mismatches = seq_bytes[i..i + pattern_len]
                .iter()
                .zip(pattern_bytes.iter())
                .filter(|(a, b)| a != b)
                .count();

            if mismatches <= max_mismatches {
                matches.push((i, i + pattern_len, seq[i..i + pattern_len].to_string()));
            }
        }
    }

    matches
}

fn format_match(
    id: &str,
    start: usize,
    end: usize,
    matched: &str,
    is_bed: bool,
    has_strand: bool,
    strand: &str,
) -> String {
    if is_bed {
        if has_strand {
            format!("{}\t{}\t{}\t{}\t0\t{}", id, start, end, matched, strand)
        } else {
            format!("{}\t{}\t{}\t{}", id, start, end, matched)
        }
    } else {
        format!("{}:{}-{}\t{}\t{}", id, start + 1, end, matched, strand)
    }
}

fn search_record(record: &Record, cfg: &SearchConfig) -> Vec<String> {
    let allow_rc = cfg.regex.is_none()
        && !cfg.pattern_rc.is_empty()
        && cfg.pattern_is_nucleotide
        && record.is_nucleotide_sequence();

    let mut lines = Vec::new();

    let forward_matches = find_matches(&record.seq, &cfg.pattern, &cfg.regex, cfg.mismatches);
    for (start, end, matched) in forward_matches {
        lines.push(format_match(
            &record.id, start, end, &matched, cfg.bed, cfg.strand, "+",
        ));
    }

    if allow_rc {
        let rc_matches = find_matches(&record.seq, &cfg.pattern_rc, &None, cfg.mismatches);
        for (start, end, matched) in rc_matches {
            lines.push(format_match(
                &record.id, start, end, &matched, cfg.bed, cfg.strand, "-",
            ));
        }
    }

    lines
}

pub fn run(args: Args) -> anyhow::Result<()> {
    if args.threads == 0 {
        return Err(anyhow::anyhow!("--threads must be greater than 0"));
    }

    let regex = if args.regex {
        Some(Regex::new(&args.pattern)?)
    } else {
        None
    };

    let mut reader = if let Some(path) = &args.input {
        RecordReader::new(Some(path), crate::common::detect_format(Some(path), None))?
    } else {
        RecordReader::new(None, crate::common::Format::Fasta)?
    };

    let mut writer: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    let pattern = args.pattern;
    let pattern_is_nucleotide = is_nucleotide_text(&pattern);
    let pattern_rc = if args.regex {
        String::new()
    } else {
        reverse_complement(&pattern)
    };

    let cfg = SearchConfig {
        pattern,
        regex,
        bed: args.bed,
        strand: args.strand,
        mismatches: args.mismatches,
        pattern_rc,
        pattern_is_nucleotide,
    };

    let (job_tx, job_rx) = mpsc::channel::<Option<SearchJob>>();
    let (result_tx, result_rx) = mpsc::channel::<SearchResult>();
    let job_rx = Arc::new(Mutex::new(job_rx));
    let cfg = Arc::new(cfg);

    let mut workers = Vec::with_capacity(args.threads);
    for _ in 0..args.threads {
        let rx = Arc::clone(&job_rx);
        let tx = result_tx.clone();
        let cfg = Arc::clone(&cfg);
        workers.push(thread::spawn(move || {
            loop {
                let message = {
                    let guard = rx.lock().expect("search job receiver mutex poisoned");
                    guard.recv()
                };

                match message {
                    Ok(Some(job)) => {
                        let lines = search_record(&job.record, &cfg);
                        if tx
                            .send(SearchResult {
                                index: job.index,
                                lines,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }));
    }
    drop(result_tx);

    let mut produced = 0u64;
    while let Some(record) = reader.next_record()? {
        job_tx.send(Some(SearchJob {
            index: produced,
            record,
        }))?;
        produced += 1;
    }

    for _ in 0..args.threads {
        let _ = job_tx.send(None);
    }
    drop(job_tx);

    let mut next_index = 0u64;
    let mut received = 0u64;
    let mut pending: BTreeMap<u64, Vec<String>> = BTreeMap::new();

    while received < produced {
        let result = result_rx.recv()?;
        pending.insert(result.index, result.lines);
        received += 1;

        while let Some(lines) = pending.remove(&next_index) {
            for line in lines {
                writeln!(writer, "{}", line)?;
            }
            next_index += 1;
        }
    }

    for worker in workers {
        let _ = worker.join();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_search_protein_forward_only() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.txt");

        fs::write(&input, ">p1\nMKWVTFISLLFLFSSAYS\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            pattern: "WVTF".to_string(),
            regex: false,
            bed: false,
            strand: true,
            mismatches: 0,
            threads: 1,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\t+"));
    }
}
