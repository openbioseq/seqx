use clap::Parser;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::common::{Record, RecordReader, SeqWriter, detect_format};

#[derive(Parser)]
#[command(about = "Sample random sequences")]
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

    /// Number of sequences to sample
    #[arg(short = 'n', long, group = "sampling")]
    pub count: Option<usize>,

    /// Fraction of sequences to sample (0.0-1.0)
    #[arg(short = 'F', long, group = "sampling")]
    pub fraction: Option<f64>,

    /// Random seed for reproducibility
    #[arg(short, long, default_value = "42")]
    pub seed: u64,

    /// Line width for FASTA output
    #[arg(short = 'w', long, default_value = "80")]
    pub line_width: usize,
}

/// Reservoir sampling 算法 - 流式随机抽样
fn reservoir_sample(
    reader: &mut RecordReader,
    sample_size: usize,
    rng: &mut StdRng,
) -> anyhow::Result<Vec<Record>> {
    let mut reservoir = Vec::with_capacity(sample_size);
    let mut count = 0;

    while let Some(record) = reader.next_record()? {
        count += 1;

        if reservoir.len() < sample_size {
            // 填满 reservoir
            reservoir.push(record);
        } else {
            // 以 sample_size/count 的概率替换
            let j = rng.random_range(0..count);
            if j < sample_size {
                reservoir[j] = record;
            }
        }
    }

    Ok(reservoir)
}

/// 基于概率的流式抽样
fn fraction_sample(reader: &mut RecordReader, fraction: f64) -> anyhow::Result<Vec<Record>> {
    let mut selected = Vec::new();

    while let Some(record) = reader.next_record()? {
        let r: f64 = rand::random();
        if r < fraction {
            selected.push(record);
        }
    }

    Ok(selected)
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;

    // 验证 fraction
    if let Some(frac) = args.fraction {
        if frac < 0.0 || frac > 1.0 {
            return Err(anyhow::anyhow!("Fraction must be between 0.0 and 1.0"));
        }
    }

    let mut rng = StdRng::seed_from_u64(args.seed);

    // 根据模式选择抽样方法
    let samples = if let Some(count) = args.count {
        if count == 0 {
            eprintln!("Warning: Sample size is 0, no sequences will be output");
            return Ok(());
        }
        reservoir_sample(&mut reader, count, &mut rng)?
    } else if let Some(frac) = args.fraction {
        if frac == 0.0 {
            eprintln!("Warning: Fraction is 0, no sequences will be output");
            return Ok(());
        }
        fraction_sample(&mut reader, frac)?
    } else {
        return Err(anyhow::anyhow!("Please specify --count or --fraction"));
    };

    let sample_count = samples.len();

    // 输出样本
    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

    for record in samples {
        writer.write_record(&record)?;
    }

    eprintln!(
        "Sampled: {} sequences",
        if args.count.is_some() {
            format!("{}", sample_count)
        } else {
            format!("{} (fraction)", sample_count)
        }
    );

    Ok(())
}
