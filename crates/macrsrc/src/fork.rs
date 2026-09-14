//! Resource fork parsing (Inside Macintosh: "Resource Manager"), plus the
//! AppleDouble wrapper `unar` and friends emit for the fork on non-HFS disks.

use crate::{Error, Reader, Result, mac_roman};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub kind: [u8; 4],
    pub id: i16,
    pub name: Option<String>,
    pub data: Vec<u8>,
}

impl Resource {
    pub fn kind_str(&self) -> String {
        mac_roman(&self.kind)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ResourceFork {
    pub resources: Vec<Resource>,
}

impl ResourceFork {
    /// Parse a raw resource fork, or one wrapped in an AppleDouble container.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let fork = apple_double_resource_fork(bytes)?.unwrap_or(bytes);
        parse_fork(fork)
    }

    pub fn get(&self, kind: &[u8; 4], id: i16) -> Option<&Resource> {
        self.resources
            .iter()
            .find(|r| &r.kind == kind && r.id == id)
    }

    pub fn of_kind<'a>(&'a self, kind: &'a [u8; 4]) -> impl Iterator<Item = &'a Resource> + 'a {
        self.resources.iter().filter(move |r| &r.kind == kind)
    }
}

const APPLE_DOUBLE_MAGIC: u32 = 0x0005_1607;
const APPLE_SINGLE_MAGIC: u32 = 0x0005_1600;
const ENTRY_RESOURCE_FORK: u32 = 2;

/// Locate the resource-fork entry of an AppleSingle/AppleDouble file, if
/// `bytes` is one.
fn apple_double_resource_fork(bytes: &[u8]) -> Result<Option<&[u8]>> {
    let mut r = Reader::new(bytes);
    let Ok(magic) = r.u32("magic") else {
        return Ok(None);
    };
    if magic != APPLE_DOUBLE_MAGIC && magic != APPLE_SINGLE_MAGIC {
        return Ok(None);
    }
    r.skip(4 + 16, "header")?;
    let count = r.u16("entry count")?;
    for _ in 0..count {
        let id = r.u32("entry id")?;
        let offset = r.u32("entry offset")? as usize;
        let len = r.u32("entry length")? as usize;
        if id == ENTRY_RESOURCE_FORK {
            let end = offset.checked_add(len).ok_or(Error::Truncated("entry"))?;
            return bytes
                .get(offset..end)
                .map(Some)
                .ok_or(Error::Truncated("resource fork entry"));
        }
    }
    Err(Error::Unsupported(
        "AppleDouble file without a resource fork".into(),
    ))
}

fn parse_fork(fork: &[u8]) -> Result<ResourceFork> {
    let mut hdr = Reader::new(fork);
    let data_off = hdr.u32("data offset")? as usize;
    let map_off = hdr.u32("map offset")? as usize;
    let mut map = Reader::at(fork, map_off)?;
    map.skip(16 + 4 + 2 + 2, "map header")?;
    let type_list_off = map_off + usize::from(map.u16("type list offset")?);
    let name_list_off = map_off + usize::from(map.u16("name list offset")?);

    let mut types = Reader::at(fork, type_list_off)?;
    let type_count = types.u16("type count")?.wrapping_add(1);
    let mut resources = Vec::new();
    for _ in 0..type_count {
        let kind = types.bytes(4, "type")?;
        let kind = [kind[0], kind[1], kind[2], kind[3]];
        let count = types.u16("resource count")?.wrapping_add(1);
        let ref_off = type_list_off + usize::from(types.u16("ref list offset")?);
        let mut refs = Reader::at(fork, ref_off)?;
        for _ in 0..count {
            resources.push(read_ref(fork, &mut refs, kind, data_off, name_list_off)?);
        }
    }
    Ok(ResourceFork { resources })
}

fn read_ref(
    fork: &[u8],
    refs: &mut Reader<'_>,
    kind: [u8; 4],
    data_off: usize,
    name_list_off: usize,
) -> Result<Resource> {
    let id = refs.i16("id")?;
    let name_off = refs.u16("name offset")?;
    // High byte is attributes; low 24 bits are the data offset.
    let data_rel = (refs.u32("data offset")? & 0x00ff_ffff) as usize;
    refs.skip(4, "handle")?;

    let mut data = Reader::at(fork, data_off + data_rel)?;
    let len = data.u32("data length")? as usize;
    let data = data.bytes(len, "resource data")?.to_vec();

    let name = if name_off == 0xffff {
        None
    } else {
        let mut n = Reader::at(fork, name_list_off + usize::from(name_off))?;
        let len = usize::from(n.u8("name length")?);
        Some(mac_roman(n.bytes(len, "name")?))
    };
    Ok(Resource {
        kind,
        id,
        name,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_fork() -> Vec<u8> {
        // One 'TEXT' resource, id 7, name "hi", data "ab".
        let data = [0u8, 0, 0, 2, b'a', b'b'];
        let mut map = vec![0u8; 16 + 4 + 2 + 2];
        map.extend_from_slice(&[0, 28, 0, 50]); // type list at 28, name list after the ref list
        map.extend_from_slice(&[0, 0]); // type count - 1
        map.extend_from_slice(b"TEXT");
        map.extend_from_slice(&[0, 0, 0, 10]); // count-1, ref list offset from type list
        map.extend_from_slice(&[0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // id 7, name off 0, data off 0
        map.extend_from_slice(&[2, b'h', b'i']);
        let mut out = Vec::new();
        out.extend_from_slice(&256u32.to_be_bytes());
        out.extend_from_slice(&(256 + data.len() as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(&(map.len() as u32).to_be_bytes());
        out.resize(256, 0);
        out.extend_from_slice(&data);
        out.extend_from_slice(&map);
        out
    }

    #[test]
    fn parses_raw_fork() {
        let fork = ResourceFork::parse(&tiny_fork()).unwrap();
        let r = fork.get(b"TEXT", 7).unwrap();
        assert_eq!(r.name.as_deref(), Some("hi"));
        assert_eq!(r.data, b"ab");
    }

    #[test]
    fn parses_apple_double_wrapper() {
        let fork = tiny_fork();
        let mut ad = Vec::new();
        ad.extend_from_slice(&APPLE_DOUBLE_MAGIC.to_be_bytes());
        ad.extend_from_slice(&[0; 4 + 16]);
        ad.extend_from_slice(&1u16.to_be_bytes());
        ad.extend_from_slice(&ENTRY_RESOURCE_FORK.to_be_bytes());
        ad.extend_from_slice(&38u32.to_be_bytes());
        ad.extend_from_slice(&(fork.len() as u32).to_be_bytes());
        ad.extend_from_slice(&fork);
        assert!(ResourceFork::parse(&ad).unwrap().get(b"TEXT", 7).is_some());
    }
}
