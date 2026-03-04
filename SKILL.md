# seqx-cli Skill

`seqx` is a sequence-processing CLI designed for agent workflows.

It prioritizes streaming execution and supports FASTA/FASTQ (including `.gz` input).

## Installation

```bash
cargo build --release
```

Binary path: `target/release/seqx`.

## Current Capability Summary

- Parsing layer: `RecordReader` uses `noodles` for FASTA/FASTQ.
- Large-file strategy: external `sort`, two-pass `split --parts`, bucketed stable `dedup`.
- Temp records: `packed_seq_io` (2-bit packed for A/C/G/T; fallback for non-ACGT text).
- Parallel paths: multi-threaded `sort`, `dedup`, and `search` (`--threads`).
- Protein support: all commands can process protein FASTA; nucleotide-only operations are guarded.

## Core Commands

### Statistics
```bash
seqx stats -i input.fa
seqx stats -i input.fa --gc
seqx stats -i input.fq --qual
```

### Format Conversion
```bash
seqx convert -i input.fa -T fastq
seqx convert -i input.fq -T fasta
```

### Filtering
```bash
seqx filter -i input.fa --min-len 100
seqx filter -i input.fa --max-len 1000
seqx filter -i input.fa --gc-min 40 --gc-max 60
seqx filter -i input.fa --pattern "ATG.*TAA"
seqx filter -i input.fa --id-file ids.txt
```

Note: `--gc-min/--gc-max` is nucleotide-only and errors on non-nucleotide records.

### Extraction
```bash
seqx extract -i input.fa --id seq1
seqx extract -i input.fa --range 1:100
seqx extract -i input.fa --bed regions.bed
```

### Pattern Search
```bash
seqx search -i input.fa "ATG"
seqx search -i input.fa "ATG.*TAA" --regex
seqx search -i input.fa "CGTA" --strand
seqx search -i input.fa "ATG" --bed
seqx search -i input.fa "ATG" --threads 8
```

Note: reverse-complement matching is enabled only when both pattern and record are nucleotide-like.

### Sequence Modification
```bash
seqx modify -i input.fa --reverse-complement
seqx modify -i input.fa --upper
seqx modify -i input.fa --slice 1:100
seqx modify -i input.fa --remove-gaps
```

Note: `--reverse-complement` is nucleotide-only and errors on non-nucleotide records.

### Sampling
```bash
seqx sample -i input.fa -n 1000
seqx sample -i input.fa -F 0.1
seqx sample -i input.fa -n 100 --seed 42
```

### Sorting
```bash
seqx sort -i input.fa --by-len --desc
seqx sort -i input.fa --by-name
seqx sort -i input.fa --by-gc
seqx sort -i input.fa --by-name --max-memory 128
seqx sort -i input.fa --by-name --threads 8
```

### Deduplication
```bash
seqx dedup -i input.fa
seqx dedup -i input.fa --by-id
seqx dedup -i input.fa --prefix 10
seqx dedup -i input.fa --buckets 256
seqx dedup -i input.fa --buckets 256 --threads 8
```

### Merging
```bash
seqx merge a.fa b.fa c.fa -o out.fa
seqx merge *.fa -o out.fa --add-prefix
```

### Splitting
```bash
seqx split -i input.fa -n 10
seqx split -i input.fa -c 1000
seqx split -i input.fa --by-id
```

## Common Patterns

### Pipeline Usage
```bash
# Filter long reads -> convert -> sample
seqx filter -i reads.fq --min-len 1000 | seqx convert -T fasta | seqx sample -n 100

# Extract BED regions then reverse-complement
seqx extract -i genome.fa --bed regions.bed | seqx modify --reverse-complement > out.fa
```

### Parameter Reference
- `-i, --input`: input file (default `stdin`)
- `-o, --output`: output file (default `stdout`)
- `-f, --format`: force input format (`auto/fasta/fastq`)
- `-w, --line-width`: FASTA line width (default `80`)

## Format Auto-Detection

Automatic detection is extension-based:
- `.fa`, `.fasta` -> FASTA
- `.fq`, `.fastq` -> FASTQ
- `.gz` -> transparent gzip decompression

## Tips

- Prefer `-i` for script-friendly usage (except `merge`, which takes positional inputs).
- Prefer pipelines for multi-step workflows to avoid intermediate files.
- For large sorts, set `sort --max-memory` and tune `sort --threads`.
- For large dedup jobs, tune `dedup --buckets` and `dedup --threads`.
- For heavy search workloads, tune `search --threads` (default `1`).
