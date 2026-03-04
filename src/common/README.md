# Common Module Guide

This module contains shared infrastructure used by all command implementations.

## File Layout

```
common/
├── mod.rs            # re-exports
├── record.rs         # Record model + sequence methods
├── parser.rs         # format detection + streaming parser
├── packed_seq_io.rs  # binary temp-record codec
└── writer.rs         # output writer abstraction
```

## Core Components

### `record.rs`

Canonical sequence representation:

```rust
pub struct Record {
    pub id: String,
    pub desc: Option<String>,
    pub seq: String,
    pub qual: Option<Vec<u8>>,
}
```

Common methods include:

- `len()`, `is_empty()`
- `is_nucleotide_sequence()`, `is_nucleotide_text()`
- `gc_content()`, `avg_quality()`
- `reverse_complement()`, `to_upper()`, `to_lower()`, `remove_gaps()`, `slice()`
- `write_fasta()`, `write_fastq()`, `write_fasta_with_prefix()`
- `dedup_key()`

### `parser.rs`

Main responsibilities:

- `Format` enum (`Fasta`, `Fastq`, `Auto`)
- `detect_format()`
- `open_file()` with transparent gzip support
- `RecordReader` streaming reader (`next_record()`)
- `parse_records()` batch helper
- `parse_range()` for `start:end` (1-based inclusive input -> 0-based half-open output)

### `writer.rs`

- `SeqWriter` for FASTA/FASTQ output
- `create_writer()` for file/stdout writer creation

### `packed_seq_io.rs`

Binary encoding used by temporary files in large-file workflows (`sort`, `dedup`):

- `write_record_binary()` / `read_record_binary()`
- `write_u64()` / `read_u64()` helpers

Encoding behavior:

- A/C/G/T sequences prefer packed 2-bit encoding.
- Non-ACGT content (for example protein strings) uses safe fallback text encoding.

## Typical Usage Pattern

```rust
use crate::common::{detect_format, RecordReader, SeqWriter};

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    let mut reader = RecordReader::new(args.input.as_deref(), format)?;
    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

    while let Some(record) = reader.next_record()? {
        writer.write_record(&record)?;
    }

    Ok(())
}
```

## Design Goals

1. Reuse parser/writer logic across commands.
2. Keep behavior consistent for FASTA/FASTQ handling.
3. Centralize I/O behavior changes in one place.
4. Support scalable workflows with streaming and compact temp storage.
