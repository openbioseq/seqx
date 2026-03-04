pub mod packed_seq_io;
pub mod parser;
pub mod record;
pub mod writer;

pub use packed_seq_io::{
    read_record_binary, read_record_binary_from_slice, read_u64, write_record_binary, write_u64,
};
pub use parser::{
    Format, RecordReader, detect_format, open_file, parse_range, parse_records, record_iter,
};
pub use record::Record;
pub use writer::{SeqWriter, create_writer};
