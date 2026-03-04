# Common 模块说明

此模块包含所有子命令共享的功能，避免代码重复，确保一致性。

## 文件结构

```
common/
├── mod.rs      # 模块导出
├── record.rs   # Record 结构体及其方法
├── parser.rs   # 文件解析相关功能
├── packed_seq_io.rs # 临时落盘二进制编解码
└── writer.rs   # 输出写入相关功能
```

## 主要组件

### Record 结构体 (record.rs)

序列记录的统一表示：

```rust
pub struct Record {
    pub id: String,           // 序列ID
    pub desc: Option<String>, // 描述
    pub seq: String,          // 序列
    pub qual: Option<Vec<u8>>, // 质量值 (FASTQ)
}
```

提供的方法：
- `len()`, `is_empty()` - 基本属性
- `is_nucleotide_sequence()`, `is_nucleotide_text()` - 核酸识别
- `gc_content()` - GC含量计算
- `avg_quality()` - 平均质量值
- `reverse_complement()` - 反向互补
- `to_upper()`, `to_lower()` - 大小写转换
- `remove_gaps()` - 移除gap
- `slice()` - 序列截取
- `write_fasta()`, `write_fastq()` - 格式输出
- `dedup_key()` - 去重键生成

### 解析功能 (parser.rs)

- `Format` 枚举 - 格式类型定义
- `detect_format()` - 自动检测格式
- `open_file()` - 打开文件（自动处理gzip）
- `parse_fasta()` - 解析FASTA
- `parse_fastq()` - 解析FASTQ
- `parse_records()` - 通用解析入口
- `parse_range()` - 范围字符串解析

### 写入功能 (writer.rs)

- `SeqWriter` - 序列写入器
- `create_writer()` - 创建输出writer

### packed_seq_io (packed_seq_io.rs)

- `write_record_binary()` / `read_record_binary()` - 统一临时文件记录格式。
- 对 A/C/G/T 序列启用 2-bit packed 压缩；蛋白等非核酸序列自动回退字符串存储。
- 供 `sort` / `dedup` 外部流式路径复用，降低 I/O 开销并保持兼容性。

## 使用示例

```rust
use crate::common::{detect_format, parse_records, SeqWriter};

pub fn run(args: Args) -> anyhow::Result<()> {
    // 1. 检测格式
    let format = detect_format(args.input.as_deref(), args.format.as_deref());
    
    // 2. 解析记录
    let records = parse_records(args.input.as_deref(), format)?;
    
    // 3. 处理记录...
    
    // 4. 写入输出
    let mut writer = SeqWriter::from_path(
        args.output.as_deref(), 
        format, 
        args.line_width
    )?;
    
    for record in records {
        writer.write_record(&record)?;
    }
    
    Ok(())
}
```

## 优势

1. **代码复用** - 解析/写入逻辑只写一次
2. **一致性** - 所有命令使用相同的解析逻辑
3. **易于维护** - 修改一处，全局生效
4. **减少错误** - 避免重复代码中的不一致
