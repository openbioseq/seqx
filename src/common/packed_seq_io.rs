use std::io::{Read, Write};

use packed_seq::{PackedSeqVec, Seq, SeqVec, unpack_base};

use crate::common::Record;

fn write_u32(writer: &mut dyn Write, value: u32) -> anyhow::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_u32(reader: &mut dyn Read) -> anyhow::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u32_from_slice(data: &[u8], offset: &mut usize) -> anyhow::Result<u32> {
    if *offset + 4 > data.len() {
        return Err(anyhow::anyhow!("Unexpected EOF while reading u32"));
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&data[*offset..*offset + 4]);
    *offset += 4;
    Ok(u32::from_le_bytes(buf))
}

pub fn write_u64(writer: &mut dyn Write, value: u64) -> anyhow::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub fn read_u64(reader: &mut dyn Read) -> anyhow::Result<Option<u64>> {
    let mut buf = [0u8; 8];
    match reader.read_exact(&mut buf) {
        Ok(()) => Ok(Some(u64::from_le_bytes(buf))),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_bool(writer: &mut dyn Write, value: bool) -> anyhow::Result<()> {
    writer.write_all(&[if value { 1 } else { 0 }])?;
    Ok(())
}

fn read_bool(reader: &mut dyn Read) -> anyhow::Result<bool> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0] != 0)
}

fn read_bool_from_slice(data: &[u8], offset: &mut usize) -> anyhow::Result<bool> {
    if *offset >= data.len() {
        return Err(anyhow::anyhow!("Unexpected EOF while reading bool"));
    }
    let value = data[*offset] != 0;
    *offset += 1;
    Ok(value)
}

fn write_bytes(writer: &mut dyn Write, bytes: &[u8]) -> anyhow::Result<()> {
    write_u32(writer, bytes.len() as u32)?;
    writer.write_all(bytes)?;
    Ok(())
}

fn read_bytes(reader: &mut dyn Read) -> anyhow::Result<Vec<u8>> {
    let len = read_u32(reader)? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_bytes_from_slice(data: &[u8], offset: &mut usize) -> anyhow::Result<Vec<u8>> {
    let len = read_u32_from_slice(data, offset)? as usize;
    if *offset + len > data.len() {
        return Err(anyhow::anyhow!("Unexpected EOF while reading byte payload"));
    }
    let bytes = data[*offset..*offset + len].to_vec();
    *offset += len;
    Ok(bytes)
}

fn write_string(writer: &mut dyn Write, value: &str) -> anyhow::Result<()> {
    write_bytes(writer, value.as_bytes())
}

fn read_string(reader: &mut dyn Read) -> anyhow::Result<String> {
    let bytes = read_bytes(reader)?;
    Ok(String::from_utf8(bytes)?)
}

fn read_string_from_slice(data: &[u8], offset: &mut usize) -> anyhow::Result<String> {
    let bytes = read_bytes_from_slice(data, offset)?;
    Ok(String::from_utf8(bytes)?)
}

fn can_pack_dna(seq: &str) -> bool {
    seq.as_bytes()
        .iter()
        .all(|&b| matches!(b, b'A' | b'C' | b'T' | b'G' | b'a' | b'c' | b't' | b'g'))
}

fn pack_dna(seq: &str) -> Vec<u8> {
    PackedSeqVec::from_ascii(seq.as_bytes()).into_raw()
}

fn unpack_dna(raw: Vec<u8>, len: usize) -> String {
    let packed = PackedSeqVec::from_raw_parts(raw, len);
    let bytes: Vec<u8> = packed
        .as_slice()
        .iter_bp()
        .map(|b| unpack_base(b as u8))
        .collect();
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn write_record_binary(writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
    write_string(writer, &record.id)?;

    write_bool(writer, record.desc.is_some())?;
    if let Some(desc) = &record.desc {
        write_string(writer, desc)?;
    }

    let packed = can_pack_dna(&record.seq);
    write_bool(writer, packed)?;
    if packed {
        write_u32(writer, record.seq.len() as u32)?;
        let raw = pack_dna(&record.seq);
        write_bytes(writer, &raw)?;
    } else {
        write_string(writer, &record.seq)?;
    }

    write_bool(writer, record.qual.is_some())?;
    if let Some(qual) = &record.qual {
        write_bytes(writer, qual)?;
    }

    Ok(())
}

pub fn read_record_binary(reader: &mut dyn Read) -> anyhow::Result<Option<Record>> {
    let id = match read_string(reader) {
        Ok(id) => id,
        Err(e) => {
            if let Some(ioe) = e.downcast_ref::<std::io::Error>()
                && ioe.kind() == std::io::ErrorKind::UnexpectedEof
            {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let has_desc = read_bool(reader)?;
    let desc = if has_desc {
        Some(read_string(reader)?)
    } else {
        None
    };

    let is_packed = read_bool(reader)?;
    let seq = if is_packed {
        let len = read_u32(reader)? as usize;
        let raw = read_bytes(reader)?;
        unpack_dna(raw, len)
    } else {
        read_string(reader)?
    };

    let has_qual = read_bool(reader)?;
    let qual = if has_qual {
        Some(read_bytes(reader)?)
    } else {
        None
    };

    Ok(Some(Record::new(id, desc, seq, qual)))
}

pub fn read_record_binary_from_slice(
    data: &[u8],
    offset: &mut usize,
) -> anyhow::Result<Option<Record>> {
    if *offset >= data.len() {
        return Ok(None);
    }

    let id = read_string_from_slice(data, offset)?;

    let has_desc = read_bool_from_slice(data, offset)?;
    let desc = if has_desc {
        Some(read_string_from_slice(data, offset)?)
    } else {
        None
    };

    let is_packed = read_bool_from_slice(data, offset)?;
    let seq = if is_packed {
        let len = read_u32_from_slice(data, offset)? as usize;
        let raw = read_bytes_from_slice(data, offset)?;
        unpack_dna(raw, len)
    } else {
        read_string_from_slice(data, offset)?
    };

    let has_qual = read_bool_from_slice(data, offset)?;
    let qual = if has_qual {
        Some(read_bytes_from_slice(data, offset)?)
    } else {
        None
    };

    Ok(Some(Record::new(id, desc, seq, qual)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_dna_record() {
        let record = Record::new(
            "dna1".to_string(),
            Some("desc".to_string()),
            "ACGTACGT".to_string(),
            None,
        );

        let mut buf = Vec::new();
        write_record_binary(&mut buf, &record).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_record_binary(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded.id, record.id);
        assert_eq!(decoded.desc, record.desc);
        assert_eq!(decoded.seq, record.seq);
        assert_eq!(decoded.qual, record.qual);
    }

    #[test]
    fn test_roundtrip_protein_record() {
        let record = Record::new(
            "prot1".to_string(),
            Some("human_albumin_fragment".to_string()),
            "MKWVTFISLLFLFSSAYSRGVFRR".to_string(),
            None,
        );

        let mut buf = Vec::new();
        write_record_binary(&mut buf, &record).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_record_binary(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded.id, record.id);
        assert_eq!(decoded.desc, record.desc);
        assert_eq!(decoded.seq, record.seq);
        assert_eq!(decoded.qual, record.qual);
    }
}
