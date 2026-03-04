# seqx 开发指南

本文档面向希望为 seqx 贡献代码或了解项目结构的开发者。

## 项目架构

```
seqx/
├── src/
│   ├── main.rs           # 程序入口：命令行解析 + 子命令分发
│   ├── lib.rs            # 库导出：cmd + common 模块
│   ├── cmd/              # 子命令实现（每个命令一个文件）
│   │   ├── mod.rs        # 命令定义（Cli + Commands enum）
│   │   ├── stats.rs      # 统计
│   │   ├── convert.rs    # 格式转换
│   │   ├── filter.rs     # 过滤
│   │   ├── extract.rs    # 提取
│   │   ├── search.rs     # 搜索
│   │   ├── modify.rs     # 修改
│   │   ├── sample.rs     # 抽样
│   │   ├── sort.rs       # 排序
│   │   ├── dedup.rs      # 去重
│   │   ├── merge.rs      # 合并
│   │   └── split.rs      # 分割
│   └── common/           # 共享模块（核心基础设施）
│       ├── mod.rs        # 模块导出
│       ├── record.rs     # Record 结构体 + 序列操作方法
│       ├── parser.rs     # 文件解析（FASTA/FASTQ/gzip）
│       ├── packed_seq_io.rs # 临时落盘记录编解码（packed-seq）
│       └── writer.rs     # 输出写入
├── Cargo.toml
├── README.md             # 用户文档
└── DEVELOPMENT.md        # 本文件
```

## 快速开始

### 构建项目

```bash
# 开发构建
cargo build

# 发布构建（优化）
cargo build --release

# 运行测试
./target/debug/seqx stats -i test.fa
```

### 添加新命令

添加新命令只需 3 个步骤：

**1. 创建命令文件 `src/cmd/newcmd.rs`**

```rust
use crate::common::{detect_format, parse_records, SeqWriter};
use clap::Parser;

#[derive(Parser)]
#[command(about = "Brief description")]
pub struct Args {
    #[arg(short, long)]
    pub input: Option<String>,
    
    #[arg(short, long)]
    pub output: Option<String>,
    
    // 添加你的参数...
}

pub fn run(args: Args) -> anyhow::Result<()> {
    // 1. 检测格式
    let format = detect_format(args.input.as_deref(), None);
    
    // 2. 解析记录
    let records = parse_records(args.input.as_deref(), format)?;
    
    // 3. 处理记录...
    
    // 4. 写入输出
    let mut writer = SeqWriter::from_path(args.output.as_deref(), format, 80)?;
    for record in records {
        writer.write_record(&record)?;
    }
    
    Ok(())
}
```

**2. 注册命令 `src/cmd/mod.rs`**

```rust
pub mod newcmd;  // 添加这行

#[derive(Subcommand)]
pub enum Commands {
    // ... 现有命令
    
    /// Brief description
    Newcmd(newcmd::Args),  // 添加这行
}
```

**3. 添加分发 `src/main.rs`**

```rust
match cli.command {
    // ... 现有命令
    Commands::Newcmd(args) => seqx::cmd::newcmd::run(args),  // 添加这行
}
```

完成！新命令已集成。

## 流式处理架构

seqx 采用**流式处理**架构，可以处理超大文件而无需一次性加载到内存。

> 2026-03-04 更新：FASTA/FASTQ 读取已切换为 noodles 驱动；`sort` 与 `split --parts` 已改为低内存外部流式方案。

### 流式读取 (RecordReader)

```rust
use crate::common::{detect_format, RecordReader};

let format = detect_format(input_path, format_arg);
let mut reader = RecordReader::new(input_path, format)?;

// 逐条处理，内存占用恒定
while let Some(record) = reader.next_record()? {
    // 处理单条记录...
}
```

### 流式写入 (SeqWriter)

```rust
use crate::common::SeqWriter;

let mut writer = SeqWriter::from_path(output_path, format, line_width)?;

// 立即写入，不缓存
for record in records {
    writer.write_record(&record)?;
}
```

### 命令的流式适配

| 命令 | 流式支持 | 说明 |
|------|----------|------|
| stats | ✅ | 累加统计信息 |
| convert | ✅ | 直接转换输出 |
| filter | ✅ | 条件判断后输出 |
| search | ✅ | 生产者-消费者流水线匹配后输出 |
| modify | ✅ | 修改后立即输出 |
| sample | ✅ | Reservoir Sampling |
| merge | ✅ | 多文件顺序处理 |
| split | ✅ | 按块/ID分割 |
| dedup | ✅ | 磁盘分桶去重 + 序号归并 |
| sort | ✅ | 外部排序（分块 + 多路归并） |
| extract | ✅ | ID/range/BED 单次扫描流式提取 |

### 仍有状态开销的命令

**sort**: 已改为外部排序，不再要求全量加载内存。可通过 `--max-memory` 控制分块内存预算，并可通过 `--threads` 指定排序线程数（默认 `1`）。

**dedup**: 已改为磁盘分桶去重，桶内维护 `HashSet`，并在最终阶段按输入序号多路归并，整体内存可控。可通过 `--threads` 指定桶内去重并行线程数（默认 `1`）。

**extract (BED模式)**: 已改为按 `seq_id` 分组后流式提取，不再构建全量序列索引。

## Common 模块详解

### Record 结构体

所有序列数据的统一表示：

```rust
pub struct Record {
    pub id: String,               // 序列ID
    pub desc: Option<String>,     // 描述
    pub seq: String,              // 序列
    pub qual: Option<Vec<u8>>,    // 质量值（FASTQ）
}
```

**常用方法：**

```rust
// 基本属性
record.len()
record.is_empty()
record.is_nucleotide_sequence()
Record::is_nucleotide_text(text)
record.gc_content()           // 返回 GC 百分比
record.avg_quality()          // 返回平均质量值

// 序列操作
record.reverse_complement()
record.to_upper()
record.to_lower()
record.remove_gaps()
record.slice(start, end)

// 输出
record.write_fasta(writer, line_width)
record.write_fastq(writer)
record.write_fasta_with_prefix(writer, prefix, sep, line_width)

// 去重键
record.dedup_key(by_id, prefix_len, ignore_case)
```

### Parser 模块

**核心函数：**

```rust
// 格式检测
let format = detect_format(path, format_arg);

// 文件打开（自动处理 gzip）
let reader = open_file("data.fa.gz")?;

// 通用解析
let records = parse_records(Some("input.fa"), Format::Fasta)?;

// 专用解析
let records = parse_fasta(reader)?;
let records = parse_fastq(reader)?;

// 范围解析 (1:100 -> (0, 100))
let (start, end) = parse_range("1:100")?;
```

**实现说明（2026-03-04）**

- `RecordReader` 内部使用 noodles：
    - FASTA: `noodles::fasta::io::Reader`
    - FASTQ: `noodles::fastq::io::Reader`
- 对外 API 保持不变：`next_record()` 返回统一 `Record`。

**Format 枚举：**

```rust
pub enum Format {
    Fasta,
    Fastq,
    Auto,
}
```

### Writer 模块

```rust
// 创建写入器
let mut writer = SeqWriter::from_path(
    Some("output.fa"),  // None = stdout
    Format::Fasta,
    80                  // line width
)?;

// 写入记录
writer.write_record(&record)?;
writer.write_record_with_prefix(&record, "prefix", ":")?;
writer.flush()?;

// 或直接创建 writer
let mut writer = create_writer(Some("output.fa"))?;
```

### packed_seq_io 模块

- `write_record_binary()` / `read_record_binary()`：统一临时落盘记录格式。
- 对 A/C/G/T（大小写）序列优先使用 `packed-seq` 2-bit 压缩；非核酸（如蛋白）自动回退为原始字符串编码。
- `write_u64()` / `read_u64()`：辅助写入序号等元信息。

该模块被 `sort` 和 `dedup` 的外部流式路径复用，减少磁盘 I/O 并保持蛋白序列兼容。

## 设计原则

### 1. 约定优于配置

- 自动检测格式（从文件扩展名）
- 默认输入 stdin，默认输出 stdout
- 合理的默认值（如 line_width = 80）

### 2. 参数命名规范

| 短参数 | 长参数 | 用途 |
|--------|--------|------|
| `-i` | `--input` | 输入文件 |
| `-o` | `--output` | 输出文件 |
| `-f` | `--format` | 强制指定格式 |
| `-w` | `--line-width` | FASTA 行宽 |

**注意：** 避免参数冲突，检查其他命令已使用的短参数。

### 3. 错误处理

使用 `anyhow` 进行错误处理：

```rust
pub fn run(args: Args) -> anyhow::Result<()> {
    let records = parse_records(...)?;  // ? 传播错误
    
    if records.is_empty() {
        return Err(anyhow::anyhow!("No sequences found"));
    }
    
    Ok(())
}
```

### 4. 输出规范

- 正常输出到 stdout
- 统计/日志信息到 stderr（使用 `eprintln!`）

```rust
eprintln!("Processed: {} sequences", count);
```

## 蛋白序列适配约定

- 全部子命令支持蛋白 FASTA 的读取、写入和流式处理。
- 核酸专属选项需要显式保护：
    - `filter --gc-min/--gc-max`：仅核酸序列可用。
    - `modify --reverse-complement`：仅核酸序列可用。
    - `search` 反链互补：仅在“模式 + 记录”均为核酸时启用。
- 对蛋白输入，上述核酸专属路径给出明确错误或自动降级（如 `search` 仅正向）。

## 代码复用模式

### 模式 1: 简单处理（修改/排序）

```rust
let mut records = parse_records(input, format)?;

// 修改记录
for record in &mut records {
    record.reverse_complement();
}

// 输出
let mut writer = SeqWriter::from_path(output, format, line_width)?;
for record in records {
    writer.write_record(&record)?;
}
```

### 模式 2: 过滤（选择性输出）

```rust
let records = parse_records(input, format)?;
let mut writer = SeqWriter::from_path(output, format, line_width)?;

let mut kept = 0;
for record in records {
    if should_keep(&record) {
        writer.write_record(&record)?;
        kept += 1;
    }
}

eprintln!("Kept: {}/{}", kept, total);
```

### 模式 3: 多文件输出（split）

```rust
for (i, chunk) in records.chunks(chunk_size).enumerate() {
    let filename = format!("{}_{:04}.fa", prefix, i + 1);
    let mut writer = create_writer(Some(&filename))?;
    
    for record in chunk {
        write_record(record, &mut writer, format, line_width)?;
    }
}
```

### 模式 4: 需要索引（extract by ID）

```rust
let records: HashMap<String, Record> = parse_records(input, format)?
    .into_iter()
    .map(|r| (r.id.clone(), r))
    .collect();

// 通过 ID 快速查找
if let Some(record) = records.get(id) {
    // ...
}
```

## 测试方法

### 手动测试

```bash
# 创建测试数据
cat > test.fa << 'EOF'
>seq1
ATGCGATCGATCG
>seq2
CGTACGTACGTACGTACGTA
EOF

# 测试命令
cargo run -- stats -i test.fa --gc
cargo run -- convert -i test.fa -T fastq
cargo run -- filter -i test.fa --min-len 15
```

### 添加单元测试

在 `common/` 模块的适当位置添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_content() {
        let record = Record::new(
            "test".to_string(),
            None,
            "GCGC".to_string(),
            None,
        );
        assert_eq!(record.gc_content(), 100.0);
    }
}
```

## 性能考虑

### 当前实现

- 默认流式处理，内存占用与记录窗口近似常量
- `sort` 采用分块外部排序，块内使用 rayon 并行排序
- `split --parts` 采用双遍扫描，避免全量加载
- 临时落盘记录对序列字段启用 `packed-seq`（2-bit ACGT）编码，以降低磁盘 I/O

### 并行策略

- `sort`：`rayon::par_sort_by` 并行排序每个块，支持 `--threads` 指定本地线程池线程数（默认 `1`）
- `dedup`：分桶后每个桶的去重可并行执行，支持 `--threads` 指定本地线程池线程数（默认 `1`）
- `search`：采用生产者-消费者流水线（读取线程 + 多工作线程匹配 + 主线程有序写出），支持 `--threads` 指定工作线程数（默认 `1`）
- 其余命令优先维持流式低内存，避免因并行批处理导致峰值内存上升

### 大文件支持（后续可选）

1. dedup 增加磁盘分桶（hash partition）
2. BED 支持持久化索引缓存
3. 更细粒度并行流水线

## 依赖管理

核心依赖：

```toml
[dependencies]
anyhow = "1.0"      # 错误处理
clap = { version = "4.5", features = ["derive"] }  # 命令行
flate2 = "1.0"      # gzip 支持
rand = "0.8"        # 随机抽样
regex = "1.10"      # 正则表达式
```

添加新依赖时：
1. 评估是否真的需要
2. 检查是否已存在类似功能
3. 更新 DEVELOPMENT.md

## 发布流程

```bash
# 1. 版本更新
cargo bump minor  # 或使用 cargo-edit

# 2. 构建发布版本
cargo build --release

# 3. 测试
cargo test
./target/release/seqx --help

# 4. 提交
git add .
git commit -m "Release vX.Y.Z"
git tag vX.Y.Z
```

## 贡献检查清单

- [ ] 代码格式：`cargo fmt`
- [ ] 无警告：`cargo build`
- [ ] 新命令已注册到 `cmd/mod.rs` 和 `main.rs`
- [ ] 参数命名符合规范
- [ ] 使用 common 模块而非重复实现
- [ ] 错误处理使用 `anyhow::Result`
- [ ] 统计信息输出到 stderr
- [ ] README.md 已更新（如添加新功能）

## 最近完成（2026-03-04）

- [x] `common::RecordReader` 切换到 noodles 读取
- [x] `sort` 改造为外部流式排序（临时文件 + 多路归并）
- [x] `split --parts` 改造为双遍流式
- [x] `extract` 全模式流式化（ID/range/BED）
- [x] `dedup` 改造为磁盘分桶去重（保持输入顺序）
- [x] 临时落盘记录接入 `packed-seq` 编解码（sort/dedup）
- [x] 全子命令蛋白 FASTA 适配，核酸专属能力加保护
- [x] 引入 `rayon` 并行热点加速
- [x] 增加单元测试（parser/extract/sort/split）

## 当前基线摘要（维护用）

- 代码路径已以流式为主，`sort`/`dedup` 使用临时文件策略处理超大输入。
- `packed_seq_io` 是临时记录的统一编解码层：DNA 优先 2-bit，非 DNA 回退原文。
- 蛋白序列为一等输入类型；核酸专属选项必须保留显式约束（不可静默错误）。
- 当前回归状态：`cargo test` 19/19 通过。
- SIMD 专项抽取未并入当前稳定基线；后续若启用，需满足“平台检测 + 标量回退 + 结果一致性测试”。

## 常见问题

### Q: 为什么使用批量读取而非流式？

A: 简化代码，适合中小文件。如需大文件支持，后续可添加流式迭代器。

### Q: 如何支持新的文件格式（如 BAM）？

A: 在 `parser.rs` 添加新的解析函数，扩展 `Format` 枚举。

### Q: 如何处理特殊质量值格式？

A: 目前使用标准 Phred+33。如需支持其他格式，在 `record.rs` 添加转换方法。

## 参考资源

- [Rust Book](https://doc.rust-lang.org/book/)
- [Clap 文档](https://docs.rs/clap/)
- [Anyhow 文档](https://docs.rs/anyhow/)
