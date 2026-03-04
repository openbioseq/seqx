# seqx

`seqx` is an agent-friendly CLI for FASTA/FASTQ sequence processing.

It is designed around streaming I/O, predictable command behavior, and low-memory execution for large files.

## Repository Layout

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
│   │   ├── convert.rs
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
└── target/                # build artifacts (generated)
```

## Build

```bash
cargo build --release
```

Binary path:

```bash
target/release/seqx
```

## Quick Start

```bash
# Show help
seqx --help

# Basic stats
seqx stats -i input.fa

# Convert FASTA -> FASTQ
seqx convert -i input.fa -T fastq -o output.fq

# Filter short sequences
seqx filter -i input.fa --min-len 100 -o filtered.fa
```

## Commands

### `stats`

```bash
seqx stats -i input.fa
seqx stats -i input.fa --gc
seqx stats -i input.fq --qual --min-len 50
```

### `convert`

```bash
seqx convert -i input.fa -T fastq -Q 30 -o output.fq
seqx convert -i input.fq -T fasta -o output.fa
```

### `filter`

```bash
seqx filter -i input.fa --min-len 100 --max-len 2000
seqx filter -i input.fa --pattern "ATG.*TAA"
seqx filter -i input.fa --exclude-pattern "N{10,}"
seqx filter -i input.fa --id-file ids.txt
seqx filter -i input.fq --min-qual 30
```

### `extract`

```bash
seqx extract -i input.fa --id seq1
seqx extract -i input.fa --id-file ids.txt
seqx extract -i input.fa --range 1:100
seqx extract -i input.fa --bed regions.bed -F 20
```

### `search`

```bash
seqx search -i input.fa "ATG"
seqx search -i input.fa "ATG.*TAA" --regex
seqx search -i input.fa "ATG" --mismatches 1 --threads 8
seqx search -i input.fa "ATG" --bed --strand
```

### `modify`

```bash
seqx modify -i input.fa --upper
seqx modify -i input.fa --lower
seqx modify -i input.fa --slice 10:200
seqx modify -i input.fa --remove-gaps
seqx modify -i input.fa --reverse-complement
```

### `sample`

```bash
seqx sample -i input.fa --count 1000 --seed 42
seqx sample -i input.fa --fraction 0.1
```

### `sort`

```bash
seqx sort -i input.fa --by-name
seqx sort -i input.fa --by-len --desc
seqx sort -i input.fa --by-gc --max-memory 256 --threads 8
```

### `dedup`

```bash
seqx dedup -i input.fa
seqx dedup -i input.fa --by-id
seqx dedup -i input.fa --prefix 12 --ignore-case
seqx dedup -i input.fa --buckets 256 --threads 8
```

### `merge`

```bash
seqx merge a.fa b.fa c.fa -o merged.fa
seqx merge a.fa b.fa c.fa --add-prefix --sep ":" -o merged_with_source.fa
```

### `split`

```bash
seqx split -i input.fa --parts 10 -o out_dir
seqx split -i input.fa --chunk-size 1000 -o out_dir
seqx split -i input.fa --by-id -o out_dir --prefix seq
```

## Behavior Notes

- Input defaults to `stdin` where supported.
- Output defaults to `stdout` where supported.
- Format detection is extension-based (`.fa/.fasta/.fq/.fastq`, optional `.gz`).
- FASTA/FASTQ parsing uses `noodles`.
- `extract` currently supports FASTA extraction only.

## Nucleotide vs Protein Behavior

- Protein FASTA records are supported by all commands.
- Nucleotide-only operations are explicitly guarded:
  - `filter --gc-min/--gc-max`
  - `modify --reverse-complement`
  - reverse-complement matching in `search` (enabled only when both record and pattern are nucleotide)

## Performance Model

- `sort`: external chunk sort + mmap merge, configurable with `--max-memory` and `--threads`.
- `dedup`: disk bucket partitioning + per-bucket dedup + stable merge, configurable with `--buckets` and `--threads`.
- `split --parts`: two-pass streaming split (stdin may be materialized to a temp file).
- Temp binary record paths use `packed_seq_io` (2-bit packing for A/C/G/T when applicable).

## Bench Script

```bash
./scripts/bench_packed_io.sh

# Custom workload
N_RECORDS=1000000 SEQ_LEN=200 DUP_RATE=40 ./scripts/bench_packed_io.sh
```

## Developer Docs

- [DEVELOPMENT.md](DEVELOPMENT.md)
- [QUICKREF.md](QUICKREF.md)
- [src/common/README.md](src/common/README.md)

## License

MIT
