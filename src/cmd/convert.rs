use clap::Parser;

use crate::common::{Format, RecordReader, SeqWriter, detect_format};

#[derive(Parser)]
#[command(about = "Convert sequence format")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Input format (auto/fasta/fastq/sam/bam/bed)
    #[arg(short = 'F', long)]
    pub from: Option<String>,

    /// Output format (fasta/fastq/sam/bam/bed)
    #[arg(short = 'T', long, default_value = "fasta")]
    pub to: String,

    /// Default quality for FASTQ (when converting from FASTA)
    #[arg(short = 'Q', long, default_value = "30")]
    pub qual: u8,

    /// Line width for FASTA/FASTQ output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    // 确定输入格式
    let from_format = if let Some(fmt) = &args.from {
        Format::from_str(fmt).unwrap_or(Format::Fasta)
    } else {
        detect_format(args.input.as_deref(), None)
    };

    let to_format = Format::from_str(&args.to).unwrap_or(Format::Fasta);

    // 创建流式读取器和写入器
    let mut reader = RecordReader::new(args.input.as_deref(), from_format)?;
    let mut writer = SeqWriter::from_path(args.output.as_deref(), to_format, args.line_width)?;

    // 流式处理
    while let Some(mut record) = reader.next_record()? {
        // 如果输出是 FASTQ 但输入没有质量值，添加默认质量值
        if to_format.is_fastq() && record.qual.is_none() {
            let qual_str: String = std::iter::repeat((args.qual + 33) as char)
                .take(record.seq.len())
                .collect();
            record.qual = Some(qual_str.into_bytes());
        }

        writer.write_record(&record)?;
    }

    Ok(())
}
