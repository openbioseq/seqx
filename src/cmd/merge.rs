use std::path::Path;

use clap::Parser;

use crate::common::{Format, RecordReader, SeqWriter, detect_format};

#[derive(Parser)]
#[command(about = "Merge multiple sequence files")]
pub struct Args {
    /// Input files
    #[arg(required = true)]
    pub inputs: Vec<String>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Input format (auto/fasta/fastq)
    #[arg(short, long)]
    pub format: Option<String>,

    /// Add file prefix to sequence IDs
    #[arg(short, long)]
    pub add_prefix: bool,

    /// Separator for prefix
    #[arg(long, default_value = ":")]
    pub sep: String,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,
}

fn get_file_prefix(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn run(args: Args) -> anyhow::Result<()> {
    if args.inputs.is_empty() {
        return Err(anyhow::anyhow!("At least one input file is required"));
    }

    // 从第一个文件确定格式
    let format = if let Some(fmt) = &args.format {
        Format::from_str(fmt).unwrap_or(Format::Fasta)
    } else {
        detect_format(Some(&args.inputs[0]), None)
    };

    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;
    let mut total_count = 0;

    // 逐个文件流式处理
    for input_path in &args.inputs {
        let mut reader = RecordReader::new(Some(input_path), format)?;
        let prefix = get_file_prefix(input_path);

        while let Some(record) = reader.next_record()? {
            if args.add_prefix {
                writer.write_record_with_prefix(&record, &prefix, &args.sep)?;
            } else {
                writer.write_record(&record)?;
            }
            total_count += 1;
        }
    }

    eprintln!(
        "Merged: {} files, {} total sequences",
        args.inputs.len(),
        total_count
    );
    Ok(())
}
