# seqx Development Guide

This guide is for contributors who want to extend `seqx` or understand the current implementation.

## Architecture Overview

```text
seqx/
├── .github/
│   └── workflows/
│       └── release.yml
├── scripts/
│   ├── bench_packed_io.sh
│   └── gen_random_fasta.py
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cmd/
│   │   ├── mod.rs
│   │   ├── compress.rs
│   │   ├── convert.rs
│   │   ├── dedup.rs
│   │   ├── guide.rs
│   │   ├── extract.rs
│   │   ├── filter.rs
│   │   ├── merge.rs
│   │   ├── modify.rs
│   │   ├── sample.rs
│   │   ├── search.rs
│   │   ├── sort.rs
│   │   ├── split.rs
│   │   └── stats.rs
│   └── common/
│       ├── mod.rs
│       ├── parser.rs
│       ├── packed_seq_io.rs
│       ├── record.rs
│       ├── writer.rs
│       └── README.md
├── Cargo.toml
├── Cargo.lock
├── README.md
├── QUICKREF.md
├── DEVELOPMENT.md
├── SKILL.md
├── rustfmt.toml
└── target/                # build artifacts (generated)
```

## Build and Test

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run formatter
cargo fmt
```

## Command Registration Flow

Every command follows the same integration pattern.

1. Implement a command module in `src/cmd/<name>.rs`.
2. Export and register it in `src/cmd/mod.rs` (`pub mod ...` and `Commands` enum).
3. Add the dispatch arm in `src/main.rs`.

### Minimal Command Template

```rust
use clap::Parser;
use crate::common::{detect_format, RecordReader, SeqWriter};

#[derive(Parser)]
#[command(about = "Brief description")]
pub struct Args {
    #[arg(short, long)]
    pub input: Option<String>,

    #[arg(short, long)]
    pub output: Option<String>,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), None);
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;
    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, 80)?;

    while let Some(record) = reader.next_record()? {
        writer.write_record(&record)?;
    }

    Ok(())
}
```

## I/O and Parsing Model

`seqx` uses streaming readers and writers by default.

- `RecordReader` yields one `Record` at a time.
- `SeqWriter` writes output records immediately.
- Gzip input is handled transparently by `open_file`.
- Format detection uses extension-based heuristics (`.fa/.fasta/.fq/.fastq`, optional `.gz`).

### Format Support in Current Parser

`Format` currently supports:

- `fasta` / `fa`
- `fastq` / `fq`
- `auto`

`RecordReader` is backed by `noodles`:

- FASTA: `noodles::fasta::io::Reader`
- FASTQ: `noodles::fastq::io::Reader`

## Common Module Responsibilities

- `record.rs`: normalized sequence record model and sequence operations.
- `parser.rs`: format detection, gzip opening, range parsing, streaming record iteration.
- `writer.rs`: output abstraction for FASTA/FASTQ writing.
- `packed_seq_io.rs`: binary temp-record encoding used by large-file sort/dedup pipelines.

## Large-File Strategies

The current codebase avoids full in-memory loading for heavy operations:

- `sort`: external chunk sort + mmap-based multiway merge.
- `dedup`: disk bucket partitioning + per-bucket dedup + stable global merge by input order.
- `split --parts`: two-pass flow (materializes stdin to temp file if needed).
- `extract`: single-pass streaming extraction for ID/range/BED paths.
- `search`: producer/worker pipeline with ordered output merge.

## Nucleotide-Only Guards

Some features are intentionally restricted to nucleotide sequences:

- `filter --gc-min/--gc-max`
- `modify --reverse-complement`
- reverse-complement branch in `search`

When non-nucleotide records are encountered in these paths, the command returns a clear error (or skips reverse-complement search when not applicable).

## CLI Consistency Notes

- Most commands use `-i/--input`, `-o/--output`, `-f/--format`.
- `convert` uses `-F/--from` and `-T/--to`.
- `merge` takes positional input files: `seqx merge <INPUTS>...`.
- Thread count is available in `search`, `sort`, `dedup`, and `compress` via `-t/--threads`.

## Suggested Contribution Checklist

- Keep changes command-scoped and minimal.
- Prefer shared helpers in `common` over duplicated logic.
- Keep command behavior stream-friendly when possible.
- Add/adjust tests near the changed module.
- Verify `cargo fmt`, `cargo build`, and `cargo test` before submission.
