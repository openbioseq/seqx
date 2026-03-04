use std::io::{self, Write};

/// 通用序列记录结构
#[derive(Clone)]
pub struct Record {
    pub id: String,
    pub desc: Option<String>,
    pub seq: String,
    pub qual: Option<Vec<u8>>,
}

impl Record {
    pub fn new(id: String, desc: Option<String>, seq: String, qual: Option<Vec<u8>>) -> Self {
        Self {
            id,
            desc,
            seq,
            qual,
        }
    }

    pub fn len(&self) -> usize {
        self.seq.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seq.is_empty()
    }

    pub fn is_nucleotide_sequence(&self) -> bool {
        Self::is_nucleotide_text(&self.seq)
    }

    pub fn is_nucleotide_text(seq: &str) -> bool {
        !seq.is_empty()
            && seq.as_bytes().iter().all(|&b| {
                matches!(
                    b,
                    b'A' | b'C'
                        | b'G'
                        | b'T'
                        | b'U'
                        | b'N'
                        | b'a'
                        | b'c'
                        | b'g'
                        | b't'
                        | b'u'
                        | b'n'
                        | b'-'
                        | b'.'
                )
            })
    }

    /// 计算 GC 含量百分比
    pub fn gc_content(&self) -> f64 {
        let seq = self.seq.as_bytes();
        let gc_count = seq
            .iter()
            .filter(|&&b| {
                let b = b.to_ascii_uppercase();
                b == b'G' || b == b'C'
            })
            .count();
        let total = seq
            .iter()
            .filter(|&&b| {
                let b = b.to_ascii_uppercase();
                matches!(b, b'G' | b'C' | b'A' | b'T' | b'U')
            })
            .count();
        if total > 0 {
            (gc_count as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }

    /// 计算平均质量值（Phred 格式）
    pub fn avg_quality(&self) -> Option<f64> {
        self.qual.as_ref().map(|q| {
            let sum: u64 = q.iter().map(|&v| v as u64).sum();
            sum as f64 / q.len() as f64
        })
    }

    /// 反向互补
    pub fn reverse_complement(&mut self) {
        self.seq = self
            .seq
            .chars()
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
            .collect();

        if let Some(ref mut qual) = self.qual {
            qual.reverse();
        }
    }

    /// 转大写
    pub fn to_upper(&mut self) {
        self.seq = self.seq.to_ascii_uppercase();
    }

    /// 转小写
    pub fn to_lower(&mut self) {
        self.seq = self.seq.to_ascii_lowercase();
    }

    /// 移除 gaps
    pub fn remove_gaps(&mut self) {
        self.seq = self.seq.replace(['-', '.'], "");
        // 质量值也需要相应调整
        if let Some(ref mut qual) = self.qual {
            let mut new_qual = Vec::new();
            let mut q_iter = qual.iter();
            for c in self.seq.chars() {
                if c != '-' && c != '.' {
                    if let Some(&q) = q_iter.next() {
                        new_qual.push(q);
                    }
                }
            }
            *qual = new_qual;
        }
    }

    /// 截取序列
    pub fn slice(&mut self, start: usize, end: usize) {
        let start = start.min(self.seq.len());
        let end = end.min(self.seq.len());

        self.seq = self.seq[start..end].to_string();

        if let Some(ref mut qual) = self.qual {
            *qual = qual[start..end].to_vec();
        }
    }

    /// 写入 FASTA 格式
    pub fn write_fasta(&self, writer: &mut dyn Write, line_width: usize) -> io::Result<()> {
        if let Some(ref desc) = self.desc {
            writeln!(writer, ">{} {}", self.id, desc)?;
        } else {
            writeln!(writer, ">{}", self.id)?;
        }
        for chunk in self.seq.as_bytes().chunks(line_width) {
            writeln!(writer, "{}", String::from_utf8_lossy(chunk))?;
        }
        Ok(())
    }

    /// 写入 FASTQ 格式
    pub fn write_fastq(&self, writer: &mut dyn Write) -> io::Result<()> {
        if let Some(ref desc) = self.desc {
            writeln!(writer, "@{} {}", self.id, desc)?;
        } else {
            writeln!(writer, "@{}", self.id)?;
        }
        writeln!(writer, "{}", self.seq)?;
        writeln!(writer, "+")?;

        if let Some(ref qual) = self.qual {
            writeln!(writer, "{}", String::from_utf8_lossy(qual))?;
        } else {
            // 默认质量值 (Phred 40 = 'I')
            writeln!(writer, "{}", "I".repeat(self.seq.len()))?;
        }
        Ok(())
    }

    /// 带前缀写入 FASTA
    pub fn write_fasta_with_prefix(
        &self,
        writer: &mut dyn Write,
        prefix: &str,
        sep: &str,
        line_width: usize,
    ) -> io::Result<()> {
        writeln!(writer, ">{}{}{}", prefix, sep, self.id)?;
        for chunk in self.seq.as_bytes().chunks(line_width) {
            writeln!(writer, "{}", String::from_utf8_lossy(chunk))?;
        }
        Ok(())
    }

    /// 获取去重键
    pub fn dedup_key(&self, by_id: bool, prefix: Option<usize>, ignore_case: bool) -> String {
        let key = if by_id {
            self.id.clone()
        } else if let Some(n) = prefix {
            self.seq.chars().take(n).collect()
        } else {
            self.seq.clone()
        };

        if ignore_case {
            key.to_ascii_uppercase()
        } else {
            key
        }
    }
}
