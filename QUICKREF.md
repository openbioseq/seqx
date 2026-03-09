# seqx Quick Reference

## Project Layout

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
│   │   ├── guide.rs
│   │   ├── dedup.rs
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
└── target/              # build artifacts (generated)
```

## Add a New Command (3 Steps)

```rust
// 1) Create src/cmd/newcmd.rs
use crate::common::{detect_format, RecordReader, SeqWriter};
use clap::Parser;

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

// 2) Register it in src/cmd/mod.rs
pub mod newcmd;
// ...
Newcmd(newcmd::Args),

// 3) Dispatch it in src/main.rs
Commands::Newcmd(args) => seqx::cmd::newcmd::run(args),
```

## Core Shared API

### `Record`

```rust
record.len()
record.is_nucleotide_sequence()
Record::is_nucleotide_text(text)
record.gc_content()
record.avg_quality()
record.reverse_complement()
record.to_upper()
record.to_lower()
record.remove_gaps()
record.slice(start, end)
record.write_fasta(writer, width)
record.write_fastq(writer)
record.dedup_key(by_id, prefix, ignore_case)
```

### `parser`

```rust
detect_format(path, format_arg) -> Format
RecordReader::new(path, format)?
reader.next_record()? -> Option<Record>
parse_records(path, format)? -> Vec<Record>
parse_range("1:100")? -> (0, 100) // 0-based, half-open
```

### `packed_seq_io`

```rust
write_record_binary(writer, &record)?
read_record_binary(reader)? -> Option<Record>
```

Note: A/C/G/T data is stored in packed 2-bit form when possible; other alphabets (for example protein sequences) fall back to plain text encoding.

### `writer`

```rust
SeqWriter::from_path(path, format, width)?
writer.write_record(&record)?
create_writer(path)?
```

## Real CLI Option Patterns

- Input/output pattern (most commands): `-i/--input`, `-o/--output`
- Common format option: `-f/--format` (except `convert`, which uses `-F/--from` and `-T/--to`)
- FASTA line width: `-w/--line-width`
- Thread options where implemented: `-t/--threads` (`search`, `sort`, `dedup`, `compress`)

## Streaming Skeleton

```rust
let format = detect_format(args.input.as_deref(), args.format.as_deref());
let mut reader = RecordReader::new(args.input.as_deref(), format)?;
let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

while let Some(record) = reader.next_record()? {
    writer.write_record(&record)?;
}
```

## Smoke Test Commands

```bash
cat > test.fa << 'EOF'
>seq1
ATGCGATCGATCG
>seq2
CGTACGTACGTACGTACGTA
EOF

cargo run -- stats -i test.fa --gc
cargo run -- convert -i test.fa -T fastq
cargo run -- filter -i test.fa --min-len 15
```

## Developer Checklist

- [ ] `cargo fmt`
- [ ] `cargo build`
- [ ] New command registered in `src/cmd/mod.rs` and `src/main.rs`
- [ ] Reused `common` helpers instead of duplicating parser/writer logic
- [ ] Logs and progress messages go to `stderr` (`eprintln!`)
- [ ] Nucleotide-only operations are guarded for non-nucleotide records

## Current Implementation Summary

- Parsing: streaming `RecordReader` backed by `noodles` for FASTA/FASTQ.
- Large-file strategy: external sort, bucketed dedup, streaming split/extract/search.
- Temp storage: binary packed format in `packed_seq_io` for lower I/O overhead.
- Parallel paths: `search`, `sort`, `dedup`, and `compress` expose `--threads` (default `1` for search/sort/dedup, auto-detect for compress).
- Compression: `compress` uses `pigz` if available, otherwise `gzp` for parallel gzip.
