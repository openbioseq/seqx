use std::io::{self, BufWriter, Write};

use super::{Format, Record};

/// 序列写入器
pub struct SeqWriter {
    writer: Box<dyn Write>,
    format: Format,
    line_width: usize,
}

impl SeqWriter {
    pub fn new(writer: Box<dyn Write>, format: Format, line_width: usize) -> Self {
        Self {
            writer,
            format,
            line_width,
        }
    }

    pub fn from_path(
        path: Option<&str>,
        format: Format,
        line_width: usize,
    ) -> anyhow::Result<Self> {
        let writer: Box<dyn Write> = match path {
            Some(p) => Box::new(BufWriter::with_capacity(
                4 * 1024 * 1024,
                std::fs::File::create(p)?,
            )),
            None => Box::new(BufWriter::with_capacity(1024 * 1024, io::stdout())),
        };
        Ok(Self::new(writer, format, line_width))
    }

    pub fn write_record(&mut self, record: &Record) -> io::Result<()> {
        if self.format.is_fastq() {
            record.write_fastq(&mut self.writer)
        } else {
            record.write_fasta(&mut self.writer, self.line_width)
        }
    }

    pub fn write_record_with_prefix(
        &mut self,
        record: &Record,
        prefix: &str,
        sep: &str,
    ) -> io::Result<()> {
        if self.format.is_fastq() {
            // FASTQ 不支持前缀，直接写入
            record.write_fastq(&mut self.writer)
        } else {
            record.write_fasta_with_prefix(&mut self.writer, prefix, sep, self.line_width)
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// 创建输出 writer
pub fn create_writer(path: Option<&str>) -> anyhow::Result<Box<dyn Write>> {
    Ok(match path {
        Some(p) => Box::new(BufWriter::with_capacity(
            4 * 1024 * 1024,
            std::fs::File::create(p)?,
        )),
        None => Box::new(BufWriter::with_capacity(1024 * 1024, io::stdout())),
    })
}
