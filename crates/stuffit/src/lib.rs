//! Parser for StuffIt 5 archives ("StuffIt (c)1997-2002 Aladdin Systems")
//! with support for stored (method 0) and Arsenic (method 15) forks.
//!
//! Layout follows XADMaster's `XADStuffIt5Parser`: an 100-byte archive
//! header, then a chain of `0xA5A5A5A5` entry headers, each followed by the
//! resource fork data and then the data fork data.

#![forbid(unsafe_code)]

mod arsenic;
mod crc;
mod macroman;
mod reader;

use std::collections::HashMap;
use std::fmt;

use reader::Reader;

/// Errors from parsing or extracting an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The bytes do not start with the StuffIt 5 signature.
    NotStuffIt5,
    /// The archive ended before a structure was complete.
    Truncated,
    /// An entry header or compressed stream is malformed.
    Corrupt,
    /// The fork uses a compression method this crate does not implement.
    Unsupported(u8),
    /// The fork is password protected.
    Encrypted,
    /// The decompressed bytes did not match the stored checksum.
    Checksum { expected: u32, actual: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStuffIt5 => write!(f, "not a StuffIt 5 archive"),
            Self::Truncated => write!(f, "archive is truncated"),
            Self::Corrupt => write!(f, "archive data is corrupt"),
            Self::Unsupported(m) => write!(f, "unsupported compression method {m}"),
            Self::Encrypted => write!(f, "entry is encrypted"),
            Self::Checksum { expected, actual } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// A parsed archive: the flat list of entries in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    pub entries: Vec<Entry>,
}

/// One file or directory in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Path components joined with `/`, decoded from Mac OS Roman. A `/`
    /// inside a Mac filename is rendered as `:` as macOS does.
    pub path: String,
    pub is_dir: bool,
    pub data_fork: Option<Fork>,
    pub resource_fork: Option<Fork>,
    pub finder_type: [u8; 4],
    pub finder_creator: [u8; 4],
}

/// One fork's compressed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fork {
    /// StuffIt compression method (0 = stored, 15 = Arsenic).
    pub method: u8,
    pub compressed_len: u32,
    pub uncompressed_len: u32,
    /// IBM CRC-16 of the uncompressed bytes. Not meaningful for method 15,
    /// which carries its own CRC-32 inside the stream.
    pub crc: u16,
    offset: usize,
    encrypted: bool,
}

const SIGNATURE: &[u8] = b"StuffIt (c)1997-";
const SIGNATURE_TAIL: &[u8] = b" Aladdin Systems, Inc., http://www.aladdinsys.com/StuffIt/\r\n";
const ENTRY_ID: u32 = 0xA5A5_A5A5;
const FLAG_DIRECTORY: u8 = 0x40;
const FLAG_ENCRYPTED: u8 = 0x20;
const ARCHIVE_FLAG_ENCRYPTED: u8 = 0x80;
const KEY_LENGTH: u8 = 5;

/// Parses the archive directory without decompressing anything.
pub fn parse(bytes: &[u8]) -> Result<Archive, Error> {
    if bytes.len() < 100
        || !bytes.starts_with(SIGNATURE)
        || &bytes[20..20 + SIGNATURE_TAIL.len()] != SIGNATURE_TAIL
    {
        return Err(Error::NotStuffIt5);
    }
    let mut r = Reader::new(bytes);
    r.skip(82)?;
    let version = r.u8()?;
    let flags = r.u8()?;
    if version != 5 {
        return Err(Error::NotStuffIt5);
    }
    let _total_size = r.u32()?;
    let _unknown = r.u32()?;
    let root_entries = r.u16()?;
    let first_offset = r.u32()?;
    let archive_encrypted = flags & ARCHIVE_FLAG_ENCRYPTED != 0;

    r.seek(first_offset as usize)?;
    let mut entries = Vec::new();
    let mut dirs: HashMap<u32, String> = HashMap::new();
    let mut remaining = usize::from(root_entries);
    while remaining > 0 {
        // Placeholders do not count towards their directory's child total.
        if let Some((entry, children)) = parse_entry(&mut r, &mut dirs, archive_encrypted)? {
            remaining += children;
            remaining -= 1;
            entries.push(entry);
        }
    }
    Ok(Archive { entries })
}

/// Reads one entry header and positions the reader after its fork data.
/// Returns `None` for the empty placeholder entries that follow each
/// directory, otherwise the entry plus its number of direct children.
fn parse_entry(
    r: &mut Reader,
    dirs: &mut HashMap<u32, String>,
    archive_encrypted: bool,
) -> Result<Option<(Entry, usize)>, Error> {
    let offset = r.pos();
    if r.u32()? != ENTRY_ID {
        return Err(Error::Corrupt);
    }
    let version = r.u8()?;
    r.skip(1)?;
    let header_end = offset + usize::from(r.u16()?);
    r.skip(1)?;
    let flags = r.u8()?;
    r.skip(16)?; // creation, modification, previous offset, next offset
    let dir_offset = r.u32()?;
    let name_len = usize::from(r.u16()?);
    let _header_crc = r.u16()?;
    let data_len = r.u32()?;
    let data_comp_len = r.u32()?;
    let data_crc = r.u16()?;
    r.skip(2)?;

    let is_dir = flags & FLAG_DIRECTORY != 0;
    let entry_encrypted = archive_encrypted || flags & FLAG_ENCRYPTED != 0;
    let mut children = 0;
    let mut data_method = 0;
    let mut data_encrypted = false;
    if is_dir {
        children = usize::from(r.u16()?);
        if data_len == 0xFFFF_FFFF {
            // Placeholder that trails every directory; header only.
            r.seek(header_end)?;
            return Ok(None);
        }
    } else {
        data_method = r.u8()?;
        data_encrypted = skip_key(r, entry_encrypted && data_len != 0)?;
    }

    let name = macroman::decode(r.take(name_len)?).replace('/', ":");
    if r.pos() < header_end {
        let comment_len = usize::from(r.u16()?);
        r.skip(2 + comment_len)?;
    }

    let has_resource = r.u16()? & 1 != 0;
    r.skip(2)?;
    let finder_type = r.fourcc()?;
    let finder_creator = r.fourcc()?;
    let _finder_flags = r.u16()?;
    r.skip(if version == 1 { 22 } else { 18 })?;

    let mut resource_fork = if has_resource {
        Some(parse_resource_fork(r, entry_encrypted)?)
    } else {
        None
    };

    let data_start = r.pos();
    let parent = dirs
        .get(&dir_offset)
        .map(String::as_str)
        .unwrap_or_default();
    let path = if parent.is_empty() {
        name
    } else {
        format!("{parent}/{name}")
    };

    if is_dir {
        dirs.insert(offset as u32, path.clone());
        let entry = Entry {
            path,
            is_dir: true,
            data_fork: None,
            resource_fork: None,
            finder_type,
            finder_creator,
        };
        return Ok(Some((entry, children)));
    }

    let resource_comp_len = resource_fork
        .as_ref()
        .map_or(0, |f| f.compressed_len as usize);
    if let Some(fork) = &mut resource_fork {
        fork.offset = data_start;
    }
    let data_fork = (data_len != 0 || !has_resource).then(|| Fork {
        method: data_method,
        compressed_len: data_comp_len,
        uncompressed_len: data_len,
        crc: data_crc,
        offset: data_start + resource_comp_len,
        encrypted: data_encrypted,
    });
    r.seek(data_start + resource_comp_len + data_comp_len as usize)?;

    let entry = Entry {
        path,
        is_dir: false,
        data_fork,
        resource_fork,
        finder_type,
        finder_creator,
    };
    Ok(Some((entry, 0)))
}

/// Reads the resource fork descriptor; its `offset` is filled in later.
fn parse_resource_fork(r: &mut Reader, entry_encrypted: bool) -> Result<Fork, Error> {
    let uncompressed_len = r.u32()?;
    let compressed_len = r.u32()?;
    let crc = r.u16()?;
    r.skip(2)?;
    let method = r.u8()?;
    let encrypted = skip_key(r, entry_encrypted && uncompressed_len != 0)?;
    Ok(Fork {
        method,
        compressed_len,
        uncompressed_len,
        crc,
        offset: 0,
        encrypted,
    })
}

/// Consumes the per-fork password key, if present. Returns whether the
/// fork is encrypted.
fn skip_key(r: &mut Reader, expected: bool) -> Result<bool, Error> {
    let key_len = r.u8()?;
    if expected {
        if key_len != KEY_LENGTH {
            return Err(Error::Corrupt);
        }
        r.skip(usize::from(key_len))?;
        Ok(true)
    } else if key_len != 0 {
        Err(Error::Corrupt)
    } else {
        Ok(false)
    }
}

impl Archive {
    /// Extracts and verifies one fork. `bytes` must be the same buffer that
    /// was passed to [`parse`].
    pub fn read_fork(&self, bytes: &[u8], fork: &Fork) -> Result<Vec<u8>, Error> {
        if fork.encrypted {
            return Err(Error::Encrypted);
        }
        let end = fork
            .offset
            .checked_add(fork.compressed_len as usize)
            .ok_or(Error::Truncated)?;
        let input = bytes.get(fork.offset..end).ok_or(Error::Truncated)?;
        let expected_len = fork.uncompressed_len as usize;

        match fork.method & 0x0F {
            0 => {
                if input.len() != expected_len {
                    return Err(Error::Corrupt);
                }
                let actual = crc::crc16(input);
                if actual != fork.crc {
                    return Err(Error::Checksum {
                        expected: u32::from(fork.crc),
                        actual: u32::from(actual),
                    });
                }
                Ok(input.to_vec())
            }
            15 => arsenic::decompress(input, expected_len),
            other => Err(Error::Unsupported(other)),
        }
    }
}
