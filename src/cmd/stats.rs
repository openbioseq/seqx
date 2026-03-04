use std::io::Write;

use clap::Parser;

use crate::common::{Record, RecordReader, detect_format};

#[derive(Parser)]
#[command(about = "Statistics of sequences")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Force format (auto/fasta/fastq)
    #[arg(short, long)]
    pub format: Option<String>,

    /// Include GC content statistics
    #[arg(long)]
    pub gc: bool,

    /// Include quality statistics (FASTQ only)
    #[arg(long)]
    pub qual: bool,

    /// Minimum length threshold
    #[arg(long)]
    pub min_len: Option<usize>,

    /// Maximum length threshold
    #[arg(long)]
    pub max_len: Option<usize>,
}

#[derive(Default, Debug)]
struct Stats {
    count: usize,
    nucleotide_count: usize,
    total_len: usize,
    min_len: usize,
    max_len: usize,
    gc_count: usize,
    at_count: usize,
    n_count: usize,
    qual_count: usize,
    qual_sum: u64,
    qual_min: u8,
    qual_max: u8,
}

impl Stats {
    fn new() -> Self {
        Self {
            min_len: usize::MAX,
            qual_min: u8::MAX,
            ..Default::default()
        }
    }

    fn add_record(&mut self, record: &Record) {
        let len = record.len();
        self.count += 1;
        self.total_len += len;
        self.min_len = self.min_len.min(len);
        self.max_len = self.max_len.max(len);

        if record.is_nucleotide_sequence() {
            self.nucleotide_count += 1;
            for &base in record.seq.as_bytes() {
                match base.to_ascii_uppercase() {
                    b'G' | b'C' => self.gc_count += 1,
                    b'A' | b'T' | b'U' => self.at_count += 1,
                    b'N' => self.n_count += 1,
                    _ => {}
                }
            }
        }

        if let Some(qual) = &record.qual {
            self.qual_count += qual.len();
            for &qval in qual {
                self.qual_sum += qval as u64;
                self.qual_min = self.qual_min.min(qval);
                self.qual_max = self.qual_max.max(qval);
            }
        }
    }

    fn mean_len(&self) -> f64 {
        if self.count > 0 {
            self.total_len as f64 / self.count as f64
        } else {
            0.0
        }
    }

    fn gc_percent(&self) -> f64 {
        let total = self.gc_count + self.at_count + self.n_count;
        if total > 0 {
            (self.gc_count as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }

    fn n_percent(&self) -> f64 {
        let total = self.gc_count + self.at_count + self.n_count;
        if total > 0 {
            (self.n_count as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }

    fn mean_qual(&self) -> f64 {
        if self.qual_count > 0 {
            self.qual_sum as f64 / self.qual_count as f64
        } else {
            0.0
        }
    }
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;

    let mut stats = Stats::new();

    // 流式处理记录
    while let Some(record) = reader.next_record()? {
        let len = record.len();

        // 应用长度过滤
        if let Some(min) = args.min_len {
            if len < min {
                continue;
            }
        }
        if let Some(max) = args.max_len {
            if len > max {
                continue;
            }
        }

        stats.add_record(&record);
    }

    let mut writer: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    if stats.count == 0 {
        writeln!(writer, "No sequences found.")?;
        return Ok(());
    }

    writeln!(writer, "=== Sequence Statistics ===")?;
    writeln!(writer, "Count:        {}", stats.count)?;
    writeln!(writer, "Total length: {}", stats.total_len)?;
    writeln!(writer, "Min length:   {}", stats.min_len)?;
    writeln!(writer, "Max length:   {}", stats.max_len)?;
    writeln!(writer, "Mean length:  {:.2}", stats.mean_len())?;

    if args.gc {
        writeln!(writer)?;
        writeln!(writer, "=== Base Composition ===")?;
        if stats.nucleotide_count > 0 {
            writeln!(writer, "GC%: {:.2}%", stats.gc_percent())?;
            writeln!(writer, "N%:  {:.2}%", stats.n_percent())?;
            writeln!(writer, "GC count: {}", stats.gc_count)?;
            writeln!(writer, "AT count: {}", stats.at_count)?;
            writeln!(writer, "N count:  {}", stats.n_count)?;
            writeln!(
                writer,
                "Nucleotide records: {}/{}",
                stats.nucleotide_count, stats.count
            )?;
        } else {
            writeln!(
                writer,
                "GC statistics are not applicable: all records are non-nucleotide (e.g., protein sequences)."
            )?;
        }
    }

    if args.qual && stats.qual_count > 0 {
        writeln!(writer)?;
        writeln!(writer, "=== Quality Statistics ===")?;
        writeln!(writer, "Mean quality: {:.2}", stats.mean_qual())?;
        writeln!(writer, "Min quality:  {}", stats.qual_min)?;
        writeln!(writer, "Max quality:  {}", stats.qual_max)?;
    }

    Ok(())
}
