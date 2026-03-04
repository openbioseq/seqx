use std::io::Write;

use clap::Parser;
use tempfile::NamedTempFile;

use crate::common::{Format, RecordReader, create_writer, detect_format};

#[derive(Parser)]
#[command(about = "Split sequence file into multiple files")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output directory (default: current directory)
    #[arg(short, long, default_value = ".")]
    pub outdir: String,

    /// Output prefix
    #[arg(short, long, default_value = "split")]
    pub prefix: String,

    /// Input format (auto/fasta/fastq)
    #[arg(short, long)]
    pub format: Option<String>,

    /// Number of parts to split into
    #[arg(short = 'n', long, group = "split_mode")]
    pub parts: Option<usize>,

    /// Number of sequences per file
    #[arg(short = 'c', long, group = "split_mode")]
    pub chunk_size: Option<usize>,

    /// Split by sequence ID (one file per sequence)
    #[arg(long, group = "split_mode")]
    pub by_id: bool,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,
}

fn write_record(
    record: &crate::common::Record,
    writer: &mut dyn Write,
    format: Format,
    line_width: usize,
) -> anyhow::Result<()> {
    if format.is_fastq() {
        record.write_fastq(writer)?;
    } else {
        record.write_fasta(writer, line_width)?;
    }
    Ok(())
}

fn materialize_stdin_if_needed(
    input: Option<&str>,
    format: Format,
) -> anyhow::Result<(String, Option<NamedTempFile>)> {
    if let Some(path) = input {
        return Ok((path.to_string(), None));
    }

    let mut temp = NamedTempFile::new()?;
    let temp_path = temp.path().to_string_lossy().to_string();

    let mut reader = RecordReader::new(None, format)?;
    while let Some(record) = reader.next_record()? {
        write_record(&record, &mut temp, format, 80)?;
    }

    Ok((temp_path, Some(temp)))
}

fn count_records(input: &str, format: Format) -> anyhow::Result<usize> {
    let mut reader = RecordReader::new(Some(input), format)?;
    let mut count = 0usize;
    while reader.next_record()?.is_some() {
        count += 1;
    }
    Ok(count)
}

/// 流式分割 - 按块大小
fn split_by_chunk(
    reader: &mut RecordReader,
    chunk_size: usize,
    outdir: &str,
    prefix: &str,
    format: Format,
    line_width: usize,
) -> anyhow::Result<usize> {
    let ext = if format.is_fastq() { "fq" } else { "fa" };
    let mut file_idx = 1;
    let mut count_in_file = 0;
    let mut total = 0;
    let mut current_writer: Option<Box<dyn Write>> = None;

    while let Some(record) = reader.next_record()? {
        // 需要新文件
        if count_in_file == 0 {
            let filename = format!("{}/{}_{:04}.{}", outdir, prefix, file_idx, ext);
            current_writer = Some(create_writer(Some(&filename))?);
        }

        if let Some(ref mut writer) = current_writer {
            write_record(&record, writer, format, line_width)?;
        }

        count_in_file += 1;
        total += 1;

        // 当前文件已满
        if count_in_file >= chunk_size {
            count_in_file = 0;
            file_idx += 1;
            current_writer = None; // 关闭当前文件
        }
    }

    Ok(total)
}

/// 流式分割 - 按 ID
fn split_by_id(
    reader: &mut RecordReader,
    outdir: &str,
    prefix: &str,
    format: Format,
    line_width: usize,
) -> anyhow::Result<usize> {
    let ext = if format.is_fastq() { "fq" } else { "fa" };
    let mut total = 0;

    while let Some(record) = reader.next_record()? {
        let safe_id = record
            .id
            .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        let filename = format!("{}/{}_{}.{}", outdir, prefix, safe_id, ext);
        let mut writer = create_writer(Some(&filename))?;
        write_record(&record, &mut writer, format, line_width)?;
        total += 1;
    }

    Ok(total)
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());

    // 创建输出目录
    std::fs::create_dir_all(&args.outdir)?;

    if args.by_id {
        // 按 ID 分割
        let mut reader = RecordReader::new(args.input.as_deref(), format)?;
        let total = split_by_id(
            &mut reader,
            &args.outdir,
            &args.prefix,
            format,
            args.line_width,
        )?;
        eprintln!("Split: {} sequences into {} files", total, total);
    } else if let Some(chunk_size) = args.chunk_size {
        // 按块大小分割
        let mut reader = RecordReader::new(args.input.as_deref(), format)?;
        let total = split_by_chunk(
            &mut reader,
            chunk_size,
            &args.outdir,
            &args.prefix,
            format,
            args.line_width,
        )?;
        let num_files = (total + chunk_size - 1) / chunk_size;
        eprintln!(
            "Split: {} sequences into {} files ({} per file)",
            total, num_files, chunk_size
        );
    } else if let Some(parts) = args.parts {
        if parts == 0 {
            return Err(anyhow::anyhow!("--parts must be greater than 0"));
        }

        let (input_path, _temp_guard) = materialize_stdin_if_needed(args.input.as_deref(), format)?;
        let total = count_records(&input_path, format)?;

        if total == 0 {
            return Err(anyhow::anyhow!("No sequences found"));
        }

        let chunk_size = (total + parts - 1) / parts;
        let mut reader = RecordReader::new(Some(&input_path), format)?;
        split_by_chunk(
            &mut reader,
            chunk_size,
            &args.outdir,
            &args.prefix,
            format,
            args.line_width,
        )?;

        let actual_parts = (total + chunk_size - 1) / chunk_size;
        eprintln!("Split: {} sequences into {} parts", total, actual_parts);
    } else {
        return Err(anyhow::anyhow!(
            "Please specify --parts, --chunk-size, or --by-id"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_split_by_parts_streaming() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let outdir = dir.path().join("out");

        fs::write(&input, ">s1\nAA\n>s2\nTT\n>s3\nCC\n>s4\nGG\n>s5\nNN\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            outdir: outdir.to_string_lossy().to_string(),
            prefix: "part".to_string(),
            format: Some("fasta".to_string()),
            parts: Some(2),
            chunk_size: None,
            by_id: false,
            line_width: 80,
        })
        .unwrap();

        let f1 = fs::read_to_string(outdir.join("part_0001.fa")).unwrap();
        let f2 = fs::read_to_string(outdir.join("part_0002.fa")).unwrap();
        assert!(f1.contains(">s1"));
        assert!(f2.contains(">s5"));
    }

    #[test]
    fn test_split_by_chunk_streaming() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("in.fa");
        let outdir = dir.path().join("out2");

        fs::write(&input, ">a\nAA\n>b\nTT\n>c\nCC\n").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            outdir: outdir.to_string_lossy().to_string(),
            prefix: "chunk".to_string(),
            format: Some("fasta".to_string()),
            parts: None,
            chunk_size: Some(2),
            by_id: false,
            line_width: 80,
        })
        .unwrap();

        assert!(outdir.join("chunk_0001.fa").exists());
        assert!(outdir.join("chunk_0002.fa").exists());
    }
}
