# seqx - Agent友好的序列处理工具

`seqx` 是一个专为Agent设计的序列处理程序，遵循**约定大于配置**的原则，通过简洁的命令行参数完成复杂的序列操作。

## 设计原则

1. **约定优于配置**：合理的默认值，减少参数输入
2. **单一职责**：每个子命令对应一个独立功能
3. **自动识别**：自动识别输入格式（FASTA/FASTQ）
4. **流式处理**：支持大文件的流式处理，低内存占用
5. **管道友好**：支持stdin/stdout，便于管道操作
6. **超大文件友好**：对需全局计算的命令使用外部排序/双遍扫描，避免一次性全量加载
7. **标准库解析**：序列读取使用 noodles（FASTA/FASTQ）
8. **并行优化**：热点路径使用 rayon 并行加速

## 安装

```bash
cargo build --release
```

## 快速开始

```bash
# 查看帮助
seqx --help

# 基础统计
seqx stats input.fa

# 格式转换
seqx convert input.fa -o output.fq

# 过滤短序列
seqx filter input.fa --min-len 100
```

## 功能模块

### 1. stats - 序列统计
分析序列文件的基本统计信息。

```bash
seqx stats input.fa                    # 基础统计
seqx stats input.fa --gc               # 包含GC含量分析
seqx stats input.fa --qual             # 质量统计(FASTQ)
```

### 2. convert - 格式转换
支持各种生物信息学格式之间的转换。

```bash
seqx convert input.fa -o output.fq     # FASTA to FASTQ
seqx convert input.fq -o output.fa     # FASTQ to FASTA
seqx convert input.sam -o output.bam   # SAM to BAM
```

### 3. filter - 序列过滤
根据多种条件过滤序列。

```bash
seqx filter input.fa --min-len 100              # 按最小长度
seqx filter input.fa --max-len 1000             # 按最大长度
seqx filter input.fa --pattern "ATGC"           # 包含特定模式
seqx filter input.fa --gc-min 40 --gc-max 60    # GC含量范围
seqx filter input.fa --id-file ids.txt          # ID白名单
```

### 4. extract - 序列提取
提取特定序列或序列片段。

```bash
seqx extract input.fa --id seq1                 # 提取特定序列
seqx extract input.fa --range 1:100             # 提取1-100bp
seqx extract input.fa --bed regions.bed         # 按BED提取
```

### 5. search - 序列搜索
在序列中搜索motif或模式。

```bash
seqx search input.fa "ATG"                      # 简单搜索
seqx search input.fa "ATG.*TAA" --regex         # 正则搜索
seqx search input.fa --bed motifs.bed           # BED格式输出
seqx search input.fa "ATG" --threads 8          # 指定搜索线程数
```

### 6. modify - 序列修改
修改序列内容。

```bash
seqx modify input.fa --reverse-complement       # 反向互补
seqx modify input.fa --upper                    # 转大写
seqx modify input.fa --lower                    # 转小写
seqx modify input.fa --slice 1:100              # 截取
```

### 7. sample - 随机抽样
随机抽取序列子集。

```bash
seqx sample input.fa -n 1000                    # 抽取1000条
seqx sample input.fa -F 0.1                     # 抽取10%
seqx sample input.fa -n 100 --seed 42           # 指定随机种子
```

### 8. sort - 序列排序
按不同标准排序序列。

```bash
seqx sort input.fa --by-name                    # 按名称排序
seqx sort input.fa --by-len --desc              # 按长度降序
seqx sort input.fa --by-gc                      # 按GC含量
seqx sort input.fa --by-name                    # 默认 --threads=1（单线程）
seqx sort input.fa --by-name --max-memory 128   # 外部流式排序，内存上限128MB
seqx sort input.fa --by-name --threads 8        # 指定排序线程数
```

### 9. dedup - 去重
去除重复序列。

```bash
seqx dedup input.fa                             # 按序列去重
seqx dedup input.fa --by-id                     # 按ID去重
seqx dedup input.fa --prefix 10                 # 按前10bp去重
seqx dedup input.fa --buckets 128               # 默认 --threads=1（单线程）
seqx dedup input.fa --by-id --buckets 256       # 低内存磁盘分桶去重
seqx dedup input.fa --buckets 256 --threads 8   # 指定分桶去重线程数
```

### 10. merge - 合并文件
合并多个序列文件。

```bash
seqx merge a.fa b.fa c.fa -o out.fa             # 合并多个
seqx merge *.fa -o out.fa --add-prefix          # 添加来源前缀
```

### 11. split - 文件分割
将序列文件分割成多个小文件。

```bash
seqx split input.fa -n 10                       # 分成10份
seqx split input.fa -c 1000                     # 每份1000条
seqx split input.fa --by-id --prefix out_       # 按ID分割
```

## 通用参数

所有子命令都支持以下参数：

```
-i, --input <FILE>      # 输入文件(默认: stdin)
-o, --output <FILE>     # 输出文件(默认: stdout)
-f, --format <FORMAT>   # 强制指定格式(auto/fasta/fastq/sam/bam/bed)
-v, --verbose           # 详细输出
-q, --quiet             # 静默模式
```

## 输入输出约定

- 未指定输入时从stdin读取
- 未指定输出时写入stdout
- 自动识别格式（通过文件扩展名和内容）
- 支持gzip压缩文件(.gz)
- FASTA/FASTQ 读取基于 noodles，确保解析行为稳定一致
- 支持蛋白 FASTA：非 A/C/G/T 序列会自动走非 packed 回退路径，确保不丢失字符信息

## 蛋白序列支持说明

- 所有子命令均支持蛋白 FASTA 的读写与流式处理。
- 核酸专属功能会做保护：
  - `filter --gc-min/--gc-max` 仅对核酸序列有效，遇到蛋白会给出明确错误。
  - `modify --reverse-complement` 仅对核酸序列有效，遇到蛋白会给出明确错误。
  - `search` 的反链互补搜索仅在“模式 + 序列”均为核酸时启用；蛋白仅做正向搜索。
- 其他通用能力（长度、模式匹配、抽样、排序、去重、合并、分割、提取、转换）可直接用于蛋白序列。

## 内存与并行策略

- `sort` 使用外部流式排序（分块 + 临时文件 + 多路归并），支持 `--max-memory` 控制块内存目标，支持 `--threads` 指定排序线程数（默认 `1`）。
- `split --parts` 使用双遍流式扫描（必要时临时文件物化 stdin），避免全量驻留内存。
- `extract`（含 `--bed` / `--id + --range`）统一单次扫描流式提取。
- `search` 使用生产者-消费者多线程流水线（读取/匹配/有序写出），支持 `--threads` 指定搜索线程数（默认 `1`）。
- `dedup` 使用磁盘分桶 + 桶内去重 + 按输入序号归并，显著降低唯一键集合带来的内存峰值，并支持 `--threads` 指定桶内去重线程数（默认 `1`）。
- 所有临时落盘记录中的序列字段优先使用 `packed-seq` 2-bit 编码（ACGT），减少磁盘 I/O 体积与读取时间。

## 示例工作流

```bash
# 过滤高质量长读长，转换为FASTA，并统计
seqx filter reads.fq --min-len 1000 --min-qual 30 \
  | seqx convert - -o - \
  | seqx stats -

# 提取特定区域，反向互补，保存
seqx extract genome.fa --bed regions.bed \
  | seqx modify - --reverse-complement \
  > extracted_rc.fa
```

## packed-seq I/O 基准

可以使用内置脚本快速评估临时落盘路径（`sort` / `dedup`）的耗时与输出体积：

```bash
./scripts/bench_packed_io.sh

# 自定义规模
N_RECORDS=1000000 SEQ_LEN=200 DUP_RATE=40 ./scripts/bench_packed_io.sh
```

输出会给出输入/输出文件大小与两个命令的总耗时，便于对比不同参数下的 I/O 开销。

## 开发文档

- **[DEVELOPMENT.md](DEVELOPMENT.md)** - 完整的开发指南，包含架构说明、添加新命令的步骤、API 参考
- **[QUICKREF.md](QUICKREF.md)** - 快速参考卡片，适合日常开发查阅
- **[src/common/README.md](src/common/README.md)** - Common 模块详细说明

## License

MIT
