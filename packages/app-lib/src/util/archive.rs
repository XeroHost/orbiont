//! Resource budgets applied before archive readers allocate their central directory.
use crate::state::content_store::{input, validate_relative};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
pub const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 20_000;
pub const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

pub fn preflight<R: Read + Seek>(reader: &mut R) -> crate::Result<()> {
    let size = reader.seek(SeekFrom::End(0))?;
    if size > MAX_ARCHIVE_BYTES {
        return Err(input("Archive exceeds the compressed size limit"));
    }
    let tail_len = size.min(65_557);
    reader.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0; tail_len as usize];
    reader.read_exact(&mut tail)?;
    let offset = tail
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(offset, window)| {
            if window != b"PK\x05\x06" || offset + 22 > tail.len() {
                return None;
            }
            let comment =
                u16::from_le_bytes([tail[offset + 20], tail[offset + 21]])
                    as usize;
            (offset + 22 + comment == tail.len()).then_some(offset)
        })
        .ok_or_else(|| input("Invalid ZIP end record"))?;
    let end = &tail[offset..];
    if end.len() < 22 {
        return Err(input("Invalid ZIP end record"));
    }
    let u16_at = |i| u16::from_le_bytes([end[i], end[i + 1]]);
    let u32_at =
        |i| u32::from_le_bytes([end[i], end[i + 1], end[i + 2], end[i + 3]]);
    let count = u16_at(10) as usize;
    if u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) != u16_at(10)
        || count > MAX_ARCHIVE_ENTRIES
        || u32_at(12) > 16 * 1024 * 1024
        || u32_at(16) == u32::MAX
        || 22 + u16_at(20) as usize != end.len()
    {
        return Err(input(
            "Archive central directory exceeds limits or uses unsupported ZIP64/multi-disk format",
        ));
    }
    let central_start = u32_at(16) as u64;
    let central_size = u32_at(12) as u64;
    let end_record_start = size - tail_len + offset as u64;
    let central_end = central_start
        .checked_add(central_size)
        .ok_or_else(|| input("ZIP central directory overflow"))?;
    if central_end != end_record_start {
        return Err(input(
            "ZIP central directory range does not match its end record",
        ));
    }
    reader.seek(SeekFrom::Start(central_start))?;
    let mut position = central_start;
    for _ in 0..count {
        if position.checked_add(46).is_none_or(|end| end > central_end) {
            return Err(input(
                "ZIP central directory header exceeds its range",
            ));
        }
        let mut header = [0_u8; 46];
        reader.read_exact(&mut header)?;
        if &header[..4] != b"PK\x01\x02" {
            return Err(input("Invalid ZIP central directory header"));
        }
        let length_at = |index| {
            u16::from_le_bytes([header[index], header[index + 1]]) as u64
        };
        let variable_size = length_at(28) + length_at(30) + length_at(32);
        position = position
            .checked_add(46 + variable_size)
            .ok_or_else(|| input("ZIP central directory overflow"))?;
        if position > central_end || position - central_start > 16 * 1024 * 1024
        {
            return Err(input(
                "ZIP central directory variable fields exceed its range or budget",
            ));
        }
        reader.seek(SeekFrom::Start(position))?;
    }
    if position != central_end {
        return Err(input(
            "ZIP central directory count/size does not match actual headers",
        ));
    }
    reader.seek(SeekFrom::Start(0))?;
    Ok(())
}
pub fn preflight_file(path: &Path) -> crate::Result<()> {
    preflight(&mut std::fs::File::open(path)?)
}
pub fn validate_archive_path(name: &str) -> crate::Result<()> {
    if name.starts_with('/') || name.contains('\\') {
        return Err(input("Invalid archive path"));
    }
    validate_relative(name.trim_end_matches('/'))?;
    for component in name.trim_end_matches('/').split('/') {
        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(input("Reserved device name is not a file path"));
        }
    }
    Ok(())
}

pub fn validate_entry(
    name: &str,
    size: u64,
    total: &mut u64,
) -> crate::Result<()> {
    validate_archive_path(name)?;
    *total = total
        .checked_add(size)
        .ok_or_else(|| input("Archive size overflow"))?;
    if size > MAX_ENTRY_BYTES || *total > MAX_EXPANDED_BYTES {
        return Err(input("Archive exceeds decompression limits"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dishonest_central_size_and_large_variable_fields_are_rejected_before_reader_allocation()
     {
        let mut header = vec![0_u8; 46];
        header[..4].copy_from_slice(b"PK\x01\x02");
        header[28..30].copy_from_slice(&65_535_u16.to_le_bytes());
        let mut bytes = header.clone();
        bytes.extend(vec![b'x'; 65_535]);
        let mut end = vec![0_u8; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..10].copy_from_slice(&1_u16.to_le_bytes());
        end[10..12].copy_from_slice(&1_u16.to_le_bytes());
        end[12..16].copy_from_slice(&46_u32.to_le_bytes());
        bytes.extend(&end);
        assert!(preflight(&mut std::io::Cursor::new(bytes)).is_err());
        // Even when its claimed range reaches the end record, a lying variable length is rejected.
        let mut bytes = header;
        bytes.extend(&end);
        assert!(preflight(&mut std::io::Cursor::new(bytes)).is_err());
    }
    #[test]
    fn budgets_reject_bombs_traversal_and_large_directory_before_parsing() {
        let mut total = MAX_EXPANDED_BYTES;
        assert!(validate_entry("a.txt", 1, &mut total).is_err());
        assert!(validate_entry("../outside", 1, &mut 0).is_err());
        assert!(validate_entry("a", MAX_ENTRY_BYTES + 1, &mut 0).is_err());
        let mut end = vec![0; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..10].copy_from_slice(&20_001_u16.to_le_bytes());
        end[10..12].copy_from_slice(&20_001_u16.to_le_bytes());
        assert!(preflight(&mut std::io::Cursor::new(end)).is_err());
    }
}
