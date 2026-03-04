use clap::Parser;

use crate::common::{RecordReader, SeqWriter, detect_format, parse_range};

#[derive(Parser)]
#[command(about = "Modify sequences")]
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

    /// Reverse complement
    #[arg(short = 'r', long)]
    pub reverse_complement: bool,

    /// Convert to uppercase
    #[arg(long)]
    pub upper: bool,

    /// Convert to lowercase
    #[arg(long)]
    pub lower: bool,

    /// Slice range (format: start:end, 1-based, inclusive)
    #[arg(short = 's', long)]
    pub slice: Option<String>,

    /// Remove gaps (dashes)
    #[arg(short = 'g', long)]
    pub remove_gaps: bool,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;

    // 解析截取范围
    let slice_range = args.slice.as_deref().map(parse_range).transpose()?;

    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

    // 流式处理
    while let Some(mut record) = reader.next_record()? {
        // 应用修改
        if args.reverse_complement {
            if !record.is_nucleotide_sequence() {
                return Err(anyhow::anyhow!(
                    "Reverse complement is only supported for nucleotide sequences. Non-nucleotide record: {}",
                    record.id
                ));
            }
            record.reverse_complement();
        }

        if args.upper {
            record.to_upper();
        } else if args.lower {
            record.to_lower();
        }

        if args.remove_gaps {
            record.remove_gaps();
        }

        if let Some((start, end)) = slice_range {
            record.slice(start, end);
        }

        writer.write_record(&record)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_modify_protein_upper() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">p1\nmkwvtf\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            reverse_complement: false,
            upper: true,
            lower: false,
            slice: None,
            remove_gaps: false,
            line_width: 80,
        })
        .unwrap();

        let out = fs::read_to_string(output).unwrap();
        assert!(out.contains("MKWVTF"));
    }

    #[test]
    fn test_modify_protein_reverse_complement_should_error() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("protein.fa");
        let output = dir.path().join("out.fa");

        fs::write(&input, ">p1\nMKWVTF\n").unwrap();

        let result = run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            format: Some("fasta".to_string()),
            reverse_complement: true,
            upper: false,
            lower: false,
            slice: None,
            remove_gaps: false,
            line_width: 80,
        });

        assert!(result.is_err());
    }
}
