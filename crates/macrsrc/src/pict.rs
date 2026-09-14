//! QuickDraw `PICT` version 2 decoding, limited to the subset PixelPaint-era
//! files use: a single `PackBitsRect`/`PackBitsRgn` indexed pixmap.

use crate::{Error, Reader, Result};

/// Decoded RGBA8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&px);
    }

    /// Copy a sub-rectangle.
    #[must_use]
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Self {
        let mut out = Self::new(w, h);
        for dy in 0..h {
            for dx in 0..w {
                out.set_pixel(dx, dy, self.pixel(x + dx, y + dy));
            }
        }
        out
    }

    /// Use `mask` (white = opaque, black = transparent) as this image's alpha.
    pub fn apply_mask(&mut self, mask: &Self) {
        for y in 0..self.height.min(mask.height) {
            for x in 0..self.width.min(mask.width) {
                let mut px = self.pixel(x, y);
                px[3] = mask.pixel(x, y)[0];
                self.set_pixel(x, y, px);
            }
        }
    }
}

const OP_VERSION: u16 = 0x0011;
const OP_HEADER: u16 = 0x0c00;
const OP_CLIP: u16 = 0x0001;
const OP_DEF_HILITE: u16 = 0x001e;
const OP_SHORT_COMMENT: u16 = 0x00a0;
const OP_LONG_COMMENT: u16 = 0x00a1;
const OP_PACK_BITS_RECT: u16 = 0x0098;
const OP_PACK_BITS_RGN: u16 = 0x0099;
const OP_END: u16 = 0x00ff;

/// Decode the first pixmap found in a PICT resource.
pub fn decode(pict: &[u8]) -> Result<Image> {
    let mut r = Reader::new(pict);
    r.skip(2, "pict size")?;
    let (top, left, bottom, right) = rect(&mut r)?;
    if r.u16("version op")? != OP_VERSION || r.u16("version")? != 0x02ff {
        return Err(Error::Unsupported(
            "only PICT version 2 is supported".into(),
        ));
    }
    loop {
        let op = r.u16("opcode")?;
        match op {
            OP_HEADER => r.skip(24, "header")?,
            OP_CLIP => {
                let n = usize::from(r.u16("clip size")?);
                r.skip(n - 2, "clip")?;
            }
            OP_DEF_HILITE => {}
            OP_SHORT_COMMENT => r.skip(2, "comment")?,
            OP_LONG_COMMENT => {
                r.skip(2, "comment kind")?;
                let n = usize::from(r.u16("comment size")?);
                r.skip(n, "comment")?;
            }
            OP_PACK_BITS_RECT | OP_PACK_BITS_RGN => {
                return pack_bits(&mut r, op == OP_PACK_BITS_RGN, (right - left, bottom - top));
            }
            OP_END => return Err(Error::Unsupported("PICT has no pixmap".into())),
            other => return Err(Error::Unsupported(format!("PICT opcode {other:#06x}"))),
        }
    }
}

fn rect(r: &mut Reader<'_>) -> Result<(i32, i32, i32, i32)> {
    let t = i32::from(r.i16("rect")?);
    let l = i32::from(r.i16("rect")?);
    let b = i32::from(r.i16("rect")?);
    let rt = i32::from(r.i16("rect")?);
    Ok((t, l, b, rt))
}

struct PixMap {
    row_bytes: usize,
    width: u32,
    height: u32,
    depth: u16,
    palette: Vec<[u8; 4]>,
}

fn pixmap_header(r: &mut Reader<'_>) -> Result<PixMap> {
    let row_bytes_raw = r.u16("rowBytes")?;
    let is_pixmap = row_bytes_raw & 0x8000 != 0;
    let row_bytes = usize::from(row_bytes_raw & 0x3fff);
    let (top, left, bottom, right) = rect(r)?;
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    if !is_pixmap {
        return Ok(PixMap {
            row_bytes,
            width,
            height,
            depth: 1,
            palette: vec![[255, 255, 255, 255], [0, 0, 0, 255]],
        });
    }
    r.skip(2 + 2 + 4 + 4 + 4 + 2, "pixmap fields")?;
    let depth = r.u16("pixelSize")?;
    r.skip(2 + 2 + 4 + 4 + 4, "pixmap fields")?;
    if !matches!(depth, 1 | 2 | 4 | 8) {
        return Err(Error::Unsupported(format!("{depth}-bit PICT pixmap")));
    }
    r.skip(4 + 2, "color table seed/flags")?;
    let entries = usize::from(r.u16("color table size")?) + 1;
    let mut palette = Vec::with_capacity(entries);
    for _ in 0..entries {
        r.skip(2, "color value")?;
        // 16-bit components; the high byte is the 8-bit colour.
        let red = (r.u16("red")? >> 8) as u8;
        let green = (r.u16("green")? >> 8) as u8;
        let blue = (r.u16("blue")? >> 8) as u8;
        palette.push([red, green, blue, 255]);
    }
    Ok(PixMap {
        row_bytes,
        width,
        height,
        depth,
        palette,
    })
}

fn pack_bits(r: &mut Reader<'_>, has_region: bool, frame: (i32, i32)) -> Result<Image> {
    let pm = pixmap_header(r)?;
    r.skip(8 + 8 + 2, "src/dst rect, mode")?;
    if has_region {
        let n = usize::from(r.u16("region size")?);
        r.skip(n - 2, "region")?;
    }
    // Frame is authoritative for width: rowBytes may be padded.
    let width = pm.width.min(frame.0 as u32);
    let mut img = Image::new(width, pm.height);
    let mut row = Vec::with_capacity(pm.row_bytes);
    for y in 0..pm.height {
        let n = if pm.row_bytes > 250 {
            usize::from(r.u16("row length")?)
        } else {
            usize::from(r.u8("row length")?)
        };
        row.clear();
        unpack_bits(r.bytes(n, "row data")?, &mut row);
        for x in 0..width {
            let index = pixel_index(&row, x as usize, pm.depth);
            let px = pm
                .palette
                .get(index)
                .copied()
                .ok_or(Error::Truncated("palette"))?;
            img.set_pixel(x, y, px);
        }
    }
    Ok(img)
}

fn pixel_index(row: &[u8], x: usize, depth: u16) -> usize {
    let per_byte = 8 / usize::from(depth);
    let byte = row.get(x / per_byte).copied().unwrap_or(0);
    let shift = (per_byte - 1 - x % per_byte) * usize::from(depth);
    usize::from(byte >> shift) & ((1usize << depth) - 1)
}

/// Apple PackBits run-length decoding.
pub fn unpack_bits(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
        let n = src[i];
        i += 1;
        match n {
            0..=127 => {
                let len = usize::from(n) + 1;
                let end = (i + len).min(src.len());
                out.extend_from_slice(&src[i..end]);
                i = end;
            }
            128 => {}
            _ => {
                if let Some(&b) = src.get(i) {
                    out.extend(std::iter::repeat_n(b, 257 - usize::from(n)));
                }
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_literal_and_run() {
        let mut out = Vec::new();
        unpack_bits(&[2, 1, 2, 3, 0xfe, 9], &mut out);
        assert_eq!(out, [1, 2, 3, 9, 9, 9]);
    }

    #[test]
    fn indexes_bits_msb_first() {
        assert_eq!(pixel_index(&[0b1010_0000], 0, 1), 1);
        assert_eq!(pixel_index(&[0b1010_0000], 1, 1), 0);
        assert_eq!(pixel_index(&[0x3c], 1, 4), 0xc);
        assert_eq!(pixel_index(&[7, 9], 1, 8), 9);
    }

    #[test]
    fn mask_becomes_alpha() {
        let mut img = Image::new(1, 1);
        img.set_pixel(0, 0, [1, 2, 3, 255]);
        let mut mask = Image::new(1, 1);
        mask.set_pixel(0, 0, [0, 0, 0, 255]);
        img.apply_mask(&mask);
        assert_eq!(img.pixel(0, 0), [1, 2, 3, 0]);
    }
}
