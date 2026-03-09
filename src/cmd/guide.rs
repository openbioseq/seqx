use std::io::Write;

use clap::Parser;

/// Guide information for agent usage
const COMMANDS_HELP: &[(&str, &str, &str, &[(&str, &str)], &[&str])] = &[
    (
        "compress",
        "Compress or decompress files",
        "Compress or decompress files using gzip format. Automatically uses pigz if available, otherwise uses gzp (parallel gzip in Rust) for built-in compression.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            (
                "-o, --output <FILE>",
                "Output file (default: stdout or auto-generated)",
            ),
            ("-d, --decompress", "Decompress mode (default: compress)"),
            ("-l, --level <N>", "Compression level 1-9 (default: 6)"),
            ("-f, --force", "Force overwrite of output file"),
            (
                "-k, --keep",
                "Keep input file (don't delete after processing)",
            ),
            (
                "-t, --threads <N>",
                "Number of threads for pigz (default: auto)",
            ),
            ("--no-pigz", "Force use of built-in implementation"),
            ("-v, --verbose", "Verbose output"),
        ],
        &[
            "seqx compress -i input.fa",
            "seqx compress -i input.fa -o output.fa.gz -l 9",
            "seqx compress -d -i input.fa.gz",
            "seqx compress -d -i input.fa.gz -o output.fa",
            "cat input.fa | seqx compress > output.fa.gz",
            "seqx compress -i input.fa --no-pigz -k",
        ],
    ),
    (
        "stats",
        "Statistics of sequences",
        "Calculate statistics for FASTA/FASTQ files including count, length, GC content, and quality.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Force format (auto/fasta/fastq)"),
            ("--gc", "Include GC content statistics"),
            ("--qual", "Include quality statistics (FASTQ only)"),
            ("--min-len <N>", "Minimum length threshold"),
            ("--max-len <N>", "Maximum length threshold"),
        ],
        &[
            "seqx stats -i input.fa",
            "seqx stats -i input.fa --gc",
            "seqx stats -i input.fq --qual --min-len 50",
        ],
    ),
    (
        "convert",
        "Convert sequence format",
        "Convert between FASTA and FASTQ formats. When converting to FASTQ, quality values can be specified.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            (
                "-T, --to <FORMAT>",
                "Target format: fasta or fastq (required)",
            ),
            (
                "-Q, --quality <N>",
                "Quality score for FASTA->FASTQ (default: 30)",
            ),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx convert -i input.fa -T fastq -o output.fq",
            "seqx convert -i input.fq -T fasta -o output.fa",
            "seqx convert -i input.fa -T fastq -Q 30 -o output.fq",
        ],
    ),
    (
        "filter",
        "Filter sequences",
        "Filter sequences by length, GC content, patterns, IDs, or quality scores.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("--min-len <N>", "Minimum sequence length"),
            ("--max-len <N>", "Maximum sequence length"),
            (
                "--gc-min <PCT>",
                "Minimum GC content (%%) - nucleotide only",
            ),
            (
                "--gc-max <PCT>",
                "Maximum GC content (%%) - nucleotide only",
            ),
            ("--pattern <REGEX>", "Pattern to match in sequence (regex)"),
            (
                "--exclude-pattern <REGEX>",
                "Exclude pattern in sequence (regex)",
            ),
            ("--id-file <FILE>", "File with IDs to keep (one per line)"),
            ("--id <ID>", "IDs to keep (can be used multiple times)"),
            ("--min-qual <N>", "Minimum average quality (FASTQ only)"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
            (
                "-v, --invert",
                "Invert filter (keep sequences that don't match)",
            ),
        ],
        &[
            "seqx filter -i input.fa --min-len 100 --max-len 2000",
            "seqx filter -i input.fa --pattern 'ATG.*TAA'",
            "seqx filter -i input.fa --exclude-pattern 'N{10,}'",
            "seqx filter -i input.fa --id-file ids.txt",
            "seqx filter -i input.fq --min-qual 30",
        ],
    ),
    (
        "extract",
        "Extract sequences or regions",
        "Extract specific sequences by ID or genomic regions by BED file.",
        &[
            ("-i, --input <FILE>", "Input file (FASTA only)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            (
                "--id <ID>",
                "Sequence ID to extract (can be used multiple times)",
            ),
            (
                "--id-file <FILE>",
                "File with IDs to extract (one per line)",
            ),
            ("--range <START:END>", "Extract range (1-based, inclusive)"),
            ("--bed <FILE>", "BED file with regions to extract"),
            (
                "-F, --flank <N>",
                "Flank size to add around BED regions (default: 0)",
            ),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx extract -i input.fa --id seq1",
            "seqx extract -i input.fa --id-file ids.txt",
            "seqx extract -i input.fa --range 1:100",
            "seqx extract -i input.fa --bed regions.bed -F 20",
        ],
    ),
    (
        "search",
        "Search patterns in sequences",
        "Search for patterns in sequences with support for regex, mismatches, and strand.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("--regex", "Treat pattern as regex"),
            ("--mismatches <N>", "Allow N mismatches (0-3, default: 0)"),
            (
                "--strand",
                "Search both strands (reverse complement for nucleotide)",
            ),
            ("--bed", "Output in BED format"),
            ("-t, --threads <N>", "Number of threads (default: 1)"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
            (
                "<PATTERN>",
                "Pattern to search (required, unless using --bed output)",
            ),
        ],
        &[
            "seqx search -i input.fa 'ATG'",
            "seqx search -i input.fa 'ATG.*TAA' --regex",
            "seqx search -i input.fa 'ATG' --mismatches 1 --threads 8",
            "seqx search -i input.fa 'ATG' --bed --strand",
        ],
    ),
    (
        "modify",
        "Modify sequences",
        "Modify sequences: case conversion, slicing, removing gaps, reverse complement.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("--upper", "Convert sequence to uppercase"),
            ("--lower", "Convert sequence to lowercase"),
            (
                "--slice <START:END>",
                "Extract subsequence (1-based, inclusive)",
            ),
            ("--remove-gaps", "Remove gap characters (-, .)"),
            (
                "--reverse-complement",
                "Reverse complement sequence (nucleotide only)",
            ),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx modify -i input.fa --upper",
            "seqx modify -i input.fa --lower",
            "seqx modify -i input.fa --slice 10:200",
            "seqx modify -i input.fa --remove-gaps",
            "seqx modify -i input.fa --reverse-complement",
        ],
    ),
    (
        "sample",
        "Sample random sequences",
        "Randomly sample sequences from input.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("-n, --count <N>", "Number of sequences to sample"),
            (
                "--fraction <F>",
                "Fraction of sequences to sample (0.0-1.0)",
            ),
            ("--seed <N>", "Random seed for reproducibility"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx sample -i input.fa --count 1000 --seed 42",
            "seqx sample -i input.fa --fraction 0.1",
        ],
    ),
    (
        "sort",
        "Sort sequences",
        "Sort sequences by name, length, or GC content. Supports external sorting for large files.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("--by-name", "Sort by sequence ID (default)"),
            ("--by-len", "Sort by sequence length"),
            ("--by-gc", "Sort by GC content (nucleotide only)"),
            ("--desc", "Sort in descending order"),
            (
                "--max-memory <MB>",
                "Max memory for external sort (default: 512)",
            ),
            ("-t, --threads <N>", "Number of threads (default: 1)"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx sort -i input.fa --by-name",
            "seqx sort -i input.fa --by-len --desc",
            "seqx sort -i input.fa --by-gc --max-memory 256 --threads 8",
        ],
    ),
    (
        "dedup",
        "Remove duplicate sequences",
        "Remove duplicate sequences by full sequence, prefix, or ID.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            (
                "--by-id",
                "Deduplicate by sequence ID instead of sequence content",
            ),
            ("--prefix <N>", "Compare only first N characters for dedup"),
            ("--ignore-case", "Ignore case when comparing sequences"),
            (
                "--buckets <N>",
                "Number of buckets for disk-based dedup (default: 256)",
            ),
            ("-t, --threads <N>", "Number of threads (default: 1)"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx dedup -i input.fa",
            "seqx dedup -i input.fa --by-id",
            "seqx dedup -i input.fa --prefix 12 --ignore-case",
            "seqx dedup -i input.fa --buckets 256 --threads 8",
        ],
    ),
    (
        "merge",
        "Merge multiple files",
        "Merge multiple sequence files into one.",
        &[
            ("-o, --output <FILE>", "Output file (default: stdout)"),
            ("-f, --format <FORMAT>", "Output format (auto/fasta/fastq)"),
            ("--add-prefix", "Add file prefix to sequence IDs"),
            ("--sep <CHAR>", "Separator for prefix (default: :)"),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
            ("<FILES...>", "Input files to merge (required, at least 2)"),
        ],
        &[
            "seqx merge a.fa b.fa c.fa -o merged.fa",
            "seqx merge a.fa b.fa c.fa --add-prefix --sep ':' -o merged_with_source.fa",
        ],
    ),
    (
        "split",
        "Split file into multiple files",
        "Split a sequence file into multiple files by parts, chunk size, or by ID.",
        &[
            ("-i, --input <FILE>", "Input file (default: stdin)"),
            ("-o, --output-dir <DIR>", "Output directory (required)"),
            ("-f, --format <FORMAT>", "Input format (auto/fasta/fastq)"),
            ("--parts <N>", "Split into N parts"),
            (
                "--chunk-size <N>",
                "Split by chunk size (sequences per file)",
            ),
            ("--by-id", "Split each sequence to a separate file"),
            (
                "--prefix <STR>",
                "Prefix for output filenames (default: part)",
            ),
            (
                "-w, --line-width <N>",
                "Line width for FASTA output (default: 80)",
            ),
        ],
        &[
            "seqx split -i input.fa --parts 10 -o out_dir",
            "seqx split -i input.fa --chunk-size 1000 -o out_dir",
            "seqx split -i input.fa --by-id -o out_dir --prefix seq",
        ],
    ),
];

#[derive(Parser)]
#[command(about = "Show guide information for agent usage")]
pub struct Args {
    /// Command to show help for (default: list all commands)
    pub command: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text format
    #[default]
    Text,
    /// JSON format for programmatic parsing
    Json,
    /// Markdown format
    Markdown,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    match args.format {
        OutputFormat::Text => output_text(args.command.as_deref()),
        OutputFormat::Json => output_json(args.command.as_deref()),
        OutputFormat::Markdown => output_markdown(args.command.as_deref()),
    }
}

fn output_text(command: Option<&str>) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    if let Some(cmd) = command {
        // Show detailed help for specific command
        if let Some(info) = COMMANDS_HELP.iter().find(|(name, _, _, _, _)| *name == cmd) {
            let (name, about, description, params, examples) = info;
            writeln!(handle, "=== seqx guide: {} ===", name)?;
            writeln!(handle)?;
            writeln!(handle, "About: {}", about)?;
            writeln!(handle)?;
            writeln!(handle, "Description: {}", description)?;
            writeln!(handle)?;
            writeln!(handle, "Parameters:")?;
            for (param, desc) in *params {
                writeln!(handle, "  {:25} {}", param, desc)?;
            }
            writeln!(handle)?;
            writeln!(handle, "Examples:")?;
            for example in *examples {
                writeln!(handle, "  {}", example)?;
            }
        } else {
            writeln!(handle, "Unknown command: {}", cmd)?;
            writeln!(handle, "Use 'seqx guide' to list all available commands.")?;
        }
    } else {
        // List all commands
        writeln!(
            handle,
            "=== seqx guide - Agent-friendly sequence processing tool ==="
        )?;
        writeln!(handle)?;
        writeln!(handle, "Available commands:")?;
        writeln!(handle)?;
        for (name, about, _, _, _) in COMMANDS_HELP {
            writeln!(handle, "  {:12} {}", name, about)?;
        }
        writeln!(handle)?;
        writeln!(
            handle,
            "Use 'seqx guide <command>' for detailed help on a specific command."
        )?;
        writeln!(
            handle,
            "Use 'seqx <command> --help' for full CLI help including all options."
        )?;
        writeln!(handle)?;
        writeln!(handle, "Global behavior notes:")?;
        writeln!(handle, "  - Input defaults to stdin where supported")?;
        writeln!(handle, "  - Output defaults to stdout where supported")?;
        writeln!(
            handle,
            "  - Format detection is extension-based (.fa/.fasta/.fq/.fastq, optional .gz)"
        )?;
        writeln!(
            handle,
            "  - Protein FASTA records are supported by all commands"
        )?;
        writeln!(
            handle,
            "  - Nucleotide-only operations are guarded and will error on protein sequences"
        )?;
    }

    Ok(())
}

fn output_json(command: Option<&str>) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    if let Some(cmd) = command {
        if let Some(info) = COMMANDS_HELP.iter().find(|(name, _, _, _, _)| *name == cmd) {
            let (name, about, description, params, examples) = info;
            writeln!(
                handle,
                "{}",
                serde_json::json!({
                    "command": name,
                    "about": about,
                    "description": description,
                    "parameters": params.iter().map(|(p, d)| serde_json::json!({
                        "param": p,
                        "description": d
                    })).collect::<Vec<_>>(),
                    "examples": examples
                })
            )?;
        } else {
            writeln!(
                handle,
                "{}",
                serde_json::json!({"error": format!("Unknown command: {}", cmd)})
            )?;
        }
    } else {
        let commands: Vec<_> = COMMANDS_HELP
            .iter()
            .map(|(name, about, description, params, examples)| {
                serde_json::json!({
                    "command": name,
                    "about": about,
                    "description": description,
                    "parameters": params.iter().map(|(p, d)| serde_json::json!({
                        "param": p,
                        "description": d
                    })).collect::<Vec<_>>(),
                    "examples": examples
                })
            })
            .collect();
        writeln!(
            handle,
            "{}",
            serde_json::json!({
                "tool": "seqx",
                "description": "Agent-friendly sequence processing tool",
                "commands": commands
            })
        )?;
    }

    Ok(())
}

fn output_markdown(command: Option<&str>) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    if let Some(cmd) = command {
        if let Some(info) = COMMANDS_HELP.iter().find(|(name, _, _, _, _)| *name == cmd) {
            let (name, about, description, params, examples) = info;
            writeln!(handle, "# seqx {}", name)?;
            writeln!(handle)?;
            writeln!(handle, "**{}**", about)?;
            writeln!(handle)?;
            writeln!(handle, "{}", description)?;
            writeln!(handle)?;
            writeln!(handle, "## Parameters")?;
            writeln!(handle)?;
            writeln!(handle, "| Parameter | Description |")?;
            writeln!(handle, "|-----------|-------------|")?;
            for (param, desc) in *params {
                writeln!(handle, "| `{}` | {} |", param, desc)?;
            }
            writeln!(handle)?;
            writeln!(handle, "## Examples")?;
            writeln!(handle)?;
            for example in *examples {
                writeln!(handle, "```bash")?;
                writeln!(handle, "{}", example)?;
                writeln!(handle, "```")?;
            }
        } else {
            writeln!(handle, "# Error")?;
            writeln!(handle)?;
            writeln!(handle, "Unknown command: `{}`", cmd)?;
        }
    } else {
        writeln!(handle, "# seqx Help")?;
        writeln!(handle)?;
        writeln!(handle, "Agent-friendly sequence processing tool.")?;
        writeln!(handle)?;
        writeln!(handle, "## Available Commands")?;
        writeln!(handle)?;
        for (name, about, description, _, _) in COMMANDS_HELP {
            writeln!(handle, "- **{}**: {} - {}", name, about, description)?;
        }
        writeln!(handle)?;
        writeln!(
            handle,
            "Use `seqx help <command>` for detailed help on a specific command."
        )?;
        writeln!(handle)?;
        writeln!(handle, "## Global Behavior Notes")?;
        writeln!(handle)?;
        writeln!(handle, "- Input defaults to stdin where supported")?;
        writeln!(handle, "- Output defaults to stdout where supported")?;
        writeln!(
            handle,
            "- Format detection is extension-based (`.fa`/`.fasta`/`.fq`/`.fastq`, optional `.gz`)"
        )?;
        writeln!(
            handle,
            "- Protein FASTA records are supported by all commands"
        )?;
        writeln!(
            handle,
            "- Nucleotide-only operations are guarded and will error on protein sequences"
        )?;
    }

    Ok(())
}
