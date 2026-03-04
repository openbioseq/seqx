use std::io::Write;

use clap::Parser;
use rayon::join;
use regex::Regex;

use crate::common::RecordReader;

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

fn write_match(
    writer: &mut dyn Write,
    id: &str,
    start: usize,
    end: usize,
    matched: &str,
    is_bed: bool,
    has_strand: bool,
    strand: &str,
) -> anyhow::Result<()> {
    if is_bed {
        if has_strand {
            writeln!(
                writer,
                "{}\t{}\t{}\t{}\t0\t{}",
                id, start, end, matched, strand
            )?;
        } else {
            writeln!(writer, "{}\t{}\t{}\t{}", id, start, end, matched)?;
        }
    } else {
        writeln!(
            writer,
            "{}:{}-{}\t{}\t{}",
            id,
            start + 1,
            end,
            matched,
            strand
        )?;
    }
    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<()> {
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

    let pattern_rc = if !args.regex {
        reverse_complement(&args.pattern)
    } else {
        String::new()
    };
    let pattern_is_nucleotide = is_nucleotide_text(&args.pattern);

    // 流式处理
    while let Some(record) = reader.next_record()? {
        let allow_rc = !args.regex
            && !pattern_rc.is_empty()
            && pattern_is_nucleotide
            && record.is_nucleotide_sequence();

        let (forward_matches, rc_matches) = if allow_rc {
            join(
                || find_matches(&record.seq, &args.pattern, &regex, args.mismatches),
                || find_matches(&record.seq, &pattern_rc, &None, args.mismatches),
            )
        } else {
            (
                find_matches(&record.seq, &args.pattern, &regex, args.mismatches),
                Vec::new(),
            )
        };

        for (start, end, matched) in forward_matches {
            write_match(
                &mut writer,
                &record.id,
                start,
                end,
                &matched,
                args.bed,
                args.strand,
                "+",
            )?;
        }

        for (start, end, matched) in rc_matches {
            write_match(
                &mut writer,
                &record.id,
                start,
                end,
                &matched,
                args.bed,
                args.strand,
                "-",
            )?;
        }
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
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\t+"));
    }
}
