# seqx-cli Skill

`seqx` 是面向 Agent 工作流的序列处理 CLI，优先使用流式处理，支持 FASTA/FASTQ 与 gzip 输入。

## Installation

```bash
cargo build --release
```

二进制位于 `target/release/seqx`。

## 当前能力摘要

- 读取层：`RecordReader` 使用 noodles（FASTA/FASTQ）。
- 低内存大文件：`sort` 外部排序、`split --parts` 双遍、`dedup` 分桶稳定归并。
- 临时记录：`packed_seq_io`（DNA 2-bit packed，蛋白回退原文）。
- 并行：`sort` 分块排序和 `search` 正/反链匹配使用 rayon。
- 蛋白支持：所有子命令可处理蛋白 FASTA；核酸专属能力有显式保护。

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

> 注意：`--gc-min/--gc-max` 仅核酸序列可用，蛋白输入会报错。

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
```

> 注意：反向互补搜索仅在“模式 + 序列”均为核酸时启用；蛋白只做正向。

### Sequence Modification
```bash
seqx modify -i input.fa --reverse-complement
seqx modify -i input.fa --upper
seqx modify -i input.fa --slice 1:100
seqx modify -i input.fa --remove-gaps
```

> 注意：`--reverse-complement` 仅核酸可用，蛋白输入会报错。

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
# 过滤长读长 -> 转换 -> 抽样
seqx filter -i reads.fq --min-len 1000 | seqx convert - -T fasta | seqx sample - -n 100

# BED 提取后做反向互补
seqx extract -i genome.fa --bed regions.bed | seqx modify - --reverse-complement > out.fa
```

### Parameter Reference
- `-i, --input`: 输入文件（默认 stdin）
- `-o, --output`: 输出文件（默认 stdout）
- `-f, --format`: 强制输入格式（auto/fasta/fastq）
- `-w, --line-width`: FASTA 行宽（默认 80）

## Format Auto-Detection

按扩展名自动识别：
- `.fa`, `.fasta` -> FASTA
- `.fq`, `.fastq` -> FASTQ
- `.gz` -> 自动解压

## Tips

- 输入建议使用 `-i`（而非位置参数），便于脚本化。
- 多步处理优先用管道，减少中间文件。
- 大文件排序优先设置 `sort --max-memory`，并按机器核心数设置 `sort --threads`。
- 大文件去重优先设置 `dedup --buckets`。
- 当前基线测试：`cargo test` 19/19 通过。
