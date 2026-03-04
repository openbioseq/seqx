use std::io::{self, BufRead};

use noodles::{fasta, fastq};

use super::Record;

/// 序列格式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Fasta,
    Fastq,
    Auto,
}

impl Format {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "fasta" | "fa" => Some(Format::Fasta),
            "fastq" | "fq" => Some(Format::Fastq),
            "auto" => Some(Format::Auto),
            _ => None,
        }
    }

    pub fn is_fastq(&self) -> bool {
        matches!(self, Format::Fastq)
    }
}

/// 打开文件（自动处理 gzip）
pub fn open_file(path: &str) -> anyhow::Result<Box<dyn BufRead>> {
    let file = std::fs::File::open(path)?;
    if path.ends_with(".gz") {
        Ok(Box::new(io::BufReader::new(flate2::read::GzDecoder::new(
            file,
        ))))
    } else {
        Ok(Box::new(io::BufReader::new(file)))
    }
}

/// 从文件路径或内容检测格式
pub fn detect_format(path: Option<&str>, format_arg: Option<&str>) -> Format {
    if let Some(fmt) = format_arg {
        if let Some(f) = Format::from_str(fmt) {
            return f;
        }
    }

    if let Some(path) = path {
        let lower = path.to_lowercase();
        if lower.ends_with(".fq")
            || lower.ends_with(".fastq")
            || lower.ends_with(".fq.gz")
            || lower.ends_with(".fastq.gz")
        {
            return Format::Fastq;
        }
        if lower.ends_with(".fa")
            || lower.ends_with(".fasta")
            || lower.ends_with(".fa.gz")
            || lower.ends_with(".fasta.gz")
        {
            return Format::Fasta;
        }
    }

    Format::Fasta // 默认
}

/// 解析范围字符串 (start:end, 1-based, inclusive)
pub fn parse_range(range_str: &str) -> anyhow::Result<(usize, usize)> {
    let parts: Vec<&str> = range_str.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid range format. Use start:end (1-based, inclusive)"
        ));
    }

    let start: usize = parts[0].parse()?;
    let end: usize = parts[1].parse()?;

    if start == 0 || end < start {
        return Err(anyhow::anyhow!(
            "Invalid range: start must be >= 1 and end must be >= start"
        ));
    }

    // 转换为 0-based, half-open
    Ok((start - 1, end))
}

/// 流式记录读取器
pub struct RecordReader {
    inner: ReaderInner,
}

enum ReaderInner {
    Fasta(FastaStreamReader),
    Fastq(FastqStreamReader),
}

struct FastaStreamReader {
    reader: fasta::io::Reader<Box<dyn BufRead>>,
    definition_buf: String,
    sequence_buf: Vec<u8>,
}

impl FastaStreamReader {
    fn new(reader: Box<dyn BufRead>) -> Self {
        Self {
            reader: fasta::io::Reader::new(reader),
            definition_buf: String::new(),
            sequence_buf: Vec::new(),
        }
    }

    fn next_record(&mut self) -> anyhow::Result<Option<Record>> {
        self.definition_buf.clear();
        let n = self.reader.read_definition(&mut self.definition_buf)?;
        if n == 0 {
            return Ok(None);
        }

        self.sequence_buf.clear();
        self.reader.read_sequence(&mut self.sequence_buf)?;

        let definition: fasta::record::Definition = self.definition_buf.parse()?;
        let id = String::from_utf8_lossy(definition.name().as_ref()).to_string();
        let desc = definition
            .description()
            .map(|d| String::from_utf8_lossy(d.as_ref()).to_string());
        let seq = String::from_utf8_lossy(&self.sequence_buf).to_string();

        Ok(Some(Record::new(id, desc, seq, None)))
    }
}

struct FastqStreamReader {
    reader: fastq::io::Reader<Box<dyn BufRead>>,
    record_buf: fastq::Record,
}

impl FastqStreamReader {
    fn new(reader: Box<dyn BufRead>) -> Self {
        Self {
            reader: fastq::io::Reader::new(reader),
            record_buf: fastq::Record::default(),
        }
    }

    fn next_record(&mut self) -> anyhow::Result<Option<Record>> {
        let n = self.reader.read_record(&mut self.record_buf)?;
        if n == 0 {
            return Ok(None);
        }

        let id = String::from_utf8_lossy(self.record_buf.name().as_ref()).to_string();
        let desc = if self.record_buf.description().is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(self.record_buf.description().as_ref()).to_string())
        };
        let seq = String::from_utf8_lossy(self.record_buf.sequence()).to_string();
        let qual = Some(self.record_buf.quality_scores().to_vec());

        Ok(Some(Record::new(id, desc, seq, qual)))
    }
}

impl RecordReader {
    /// 创建新的流式读取器
    pub fn new(path: Option<&str>, format: Format) -> anyhow::Result<Self> {
        let reader = match path {
            Some(p) => open_file(p)?,
            None => Box::new(io::BufReader::new(io::stdin())),
        };

        let inner = if format.is_fastq() {
            ReaderInner::Fastq(FastqStreamReader::new(reader))
        } else {
            ReaderInner::Fasta(FastaStreamReader::new(reader))
        };

        Ok(Self { inner })
    }

    /// 读取下一条记录
    pub fn next_record(&mut self) -> anyhow::Result<Option<Record>> {
        match &mut self.inner {
            ReaderInner::Fasta(reader) => reader.next_record(),
            ReaderInner::Fastq(reader) => reader.next_record(),
        }
    }
}

/// 批量解析（向后兼容，小文件使用）
pub fn parse_records(input: Option<&str>, format: Format) -> anyhow::Result<Vec<Record>> {
    let mut reader = RecordReader::new(input, format)?;
    let mut records = Vec::new();

    while let Some(record) = reader.next_record()? {
        records.push(record);
    }

    Ok(records)
}

/// 创建迭代器适配器
pub fn record_iter(
    input: Option<&str>,
    format: Format,
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<Record>>> {
    let mut reader = RecordReader::new(input, format)?;

    Ok(std::iter::from_fn(move || match reader.next_record() {
        Ok(Some(record)) => Some(Ok(record)),
        Ok(None) => None,
        Err(e) => Some(Err(e)),
    }))
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_fasta_record_reader() {
        let mut file = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut file, b">seq1 desc\nACGT\n>seq2\nTTAA\n").unwrap();

        let path = file.path().to_string_lossy().to_string();
        let mut reader = RecordReader::new(Some(&path), Format::Fasta).unwrap();

        let r1 = reader.next_record().unwrap().unwrap();
        assert_eq!(r1.id, "seq1");
        assert_eq!(r1.desc.as_deref(), Some("desc"));
        assert_eq!(r1.seq, "ACGT");

        let r2 = reader.next_record().unwrap().unwrap();
        assert_eq!(r2.id, "seq2");
        assert_eq!(r2.seq, "TTAA");
        assert!(reader.next_record().unwrap().is_none());
    }

    #[test]
    fn test_fastq_record_reader() {
        let mut file = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut file, b"@r1 d1\nACGT\n+\nIIII\n").unwrap();

        let path = file.path().to_string_lossy().to_string();
        let mut reader = RecordReader::new(Some(&path), Format::Fastq).unwrap();
        let r = reader.next_record().unwrap().unwrap();

        assert_eq!(r.id, "r1");
        assert_eq!(r.desc.as_deref(), Some("d1"));
        assert_eq!(r.seq, "ACGT");
        assert_eq!(r.qual.as_ref().unwrap(), b"IIII");
    }

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("1:100").unwrap(), (0, 100));
        assert!(parse_range("0:10").is_err());
        assert!(parse_range("10:1").is_err());
    }
}
