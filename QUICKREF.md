# seqx 快速参考

## 项目结构速览

```
src/
├── main.rs           # 入口，添加新命令到 match 语句
├── lib.rs            # 导出模块
├── cmd/              # 每个命令一个文件
│   ├── mod.rs        # 注册新命令到 Commands enum
│   └── *.rs          # 命令实现
└── common/           # 共享代码
    ├── record.rs     # Record 结构体
    ├── parser.rs     # parse_records(), detect_format()
    ├── packed_seq_io.rs # 临时落盘二进制编解码
    └── writer.rs     # SeqWriter
```

## 添加新命令（3步）

```rust
// 1. 创建 src/cmd/newcmd.rs
use crate::common::{detect_format, parse_records, SeqWriter};
use clap::Parser;

#[derive(Parser)]
#[command(about = "Description")]
pub struct Args {
    #[arg(short, long)]
    pub input: Option<String>,
    #[arg(short, long)]
    pub output: Option<String>,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let format = detect_format(args.input.as_deref(), None);
    let records = parse_records(args.input.as_deref(), format)?;
    
    // 处理...
    
    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, 80)?;
    for record in records {
        writer.write_record(&record)?;
    }
    Ok(())
}

// 2. 在 cmd/mod.rs 添加
pub mod newcmd;
#[derive(Subcommand)]
pub enum Commands {
    Newcmd(newcmd::Args),
}

// 3. 在 main.rs 添加
Commands::Newcmd(args) => seqx::cmd::newcmd::run(args),
```

## Common 模块 API

### Record
```rust
record.len()
record.is_nucleotide_sequence()
record.gc_content()           // f64
record.avg_quality()          // Option<f64>
record.reverse_complement()
record.to_upper()/to_lower()
record.remove_gaps()
record.slice(start, end)
record.write_fasta(writer, width)
record.write_fastq(writer)
record.dedup_key(by_id, prefix, ignore_case)
```

### Parser
```rust
detect_format(path, format_arg) -> Format
parse_records(input, format) -> Vec<Record>
parse_fasta(reader) -> Vec<Record>
parse_fastq(reader) -> Vec<Record>
parse_range("1:100") -> (0, 100)  // 0-based
```

### packed_seq_io
```rust
write_record_binary(writer, &record)?
read_record_binary(reader)? -> Option<Record>
```

> 说明：DNA(ACGT) 会优先 2-bit packed，蛋白序列自动回退原始字符串编码。

### Writer
```rust
SeqWriter::from_path(path, format, width)?
writer.write_record(&record)?
create_writer(path) -> Box<dyn Write>
```

## 常用参数命名

| 短参 | 长参数 | 用途 |
|------|--------|------|
| `-i` | `--input` | 输入 |
| `-o` | `--output` | 输出 |
| `-f` | `--format` | 格式 |
| `-w` | `--line-width` | 行宽 |
| `-n` | `--count` | 数量 |
| `-s` | `--seed` | 随机种子 |

## 流式处理模板

### 基本流式处理
```rust
let format = detect_format(args.input.as_deref(), args.format.as_deref());
let mut reader = RecordReader::new(args.input.as_deref(), format)?;
let mut writer = SeqWriter::from_path(args.output.as_deref(), format, args.line_width)?;

while let Some(record) = reader.next_record()? {
    // 处理记录
    writer.write_record(&record)?;
}
```

### 流式过滤
```rust
while let Some(record) = reader.next_record()? {
    if should_keep(&record) {
        writer.write_record(&record)?;
    }
}
```

### Reservoir Sampling（流式随机抽样）
```rust
let mut reservoir = Vec::with_capacity(sample_size);
let mut count = 0;

while let Some(record) = reader.next_record()? {
    count += 1;
    if reservoir.len() < sample_size {
        reservoir.push(record);
    } else {
        let j = rng.gen_range(0..count);
        if j < sample_size {
            reservoir[j] = record;
        }
    }
}
```

## 代码模板

### 过滤命令
```rust
let mut kept = 0;
for record in records {
    if condition(&record) {  // 你的条件
        writer.write_record(&record)?;
        kept += 1;
    }
}
eprintln!("Kept: {}/{}", kept, total);
```

### 修改命令
```rust
for record in &mut records {
    record.some_operation();  // 修改记录
}
```

### 多文件输出
```rust
for (i, chunk) in records.chunks(size).enumerate() {
    let path = format!("out_{:04}.fa", i);
    let mut w = create_writer(Some(&path))?;
    for r in chunk { write_record(r, &mut w, format, width)?; }
}
```

## 测试命令

```bash
# 创建测试数据
cat > test.fa << 'EOF'
>seq1
ATGCGATCGATCG
>seq2
CGTACGTACGTACGTACGTA
EOF

# 快速测试
cargo run -- stats -i test.fa
cargo run -- convert -i test.fa -T fastq
cargo run -- filter -i test.fa --min-len 15
```

## 检查清单

- [ ] `cargo fmt` 格式化
- [ ] `cargo build` 无警告
- [ ] 新命令已注册到 `mod.rs` 和 `main.rs`
- [ ] 使用 common 模块而非重复代码
- [ ] 日志输出到 stderr (`eprintln!`)
- [ ] 核酸专属参数在蛋白输入下有明确保护（错误或降级）

## 当前实现摘要

- 解析：`RecordReader` 基于 noodles（FASTA/FASTQ）。
- 大文件：`sort` 外部排序、`split --parts` 双遍、`extract` 单遍流式。
- 去重：`dedup` 分桶落盘 + 稳定归并，低内存。
- 临时记录：`packed_seq_io` 统一编码（DNA 2-bit，蛋白回退）。
- 并行：`sort` 分块排序、`dedup` 桶内去重与 `search` 生产者-消费者流水线匹配均支持多线程；`sort`/`dedup`/`search` 可用 `--threads` 指定线程数（默认 `1`）。
- 现状：测试基线 `cargo test` 为 19/19 通过。

## sort 线程参数

```bash
seqx sort -i input.fa --by-name --threads 8
seqx sort -i input.fa --by-len --desc --max-memory 256 --threads 16
# 不传 --threads 时默认 1（单线程）
```

## dedup 线程参数

```bash
seqx dedup -i input.fa --buckets 128 --threads 8
seqx dedup -i input.fa --by-id --buckets 256 --threads 16
# 不传 --threads 时默认 1（单线程）
```

## search 线程参数

```bash
seqx search -i input.fa "ATG" --threads 8
seqx search -i input.fa "ATG.*TAA" --regex --threads 4
# 不传 --threads 时默认 1（单线程）
```
