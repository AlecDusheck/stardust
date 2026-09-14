//! StuffIt compression method 15 ("Arsenic"): an adaptive arithmetic coder
//! over a selector/MTF alphabet, feeding an inverse BWT, optional bit
//! "randomization", and a run-length stage. Ported from XADMaster's
//! `XADStuffItArsenicHandle`.

use crate::Error;
use crate::crc::crc32_update;

/// MSB-first bit reader; reads past the end yield zero bits so the
/// arithmetic decoder can drain its final code word.
struct BitReader<'a> {
    bytes: &'a [u8],
    bitpos: usize,
}

impl BitReader<'_> {
    fn bit(&mut self) -> u32 {
        let byte = self.bytes.get(self.bitpos / 8).copied().unwrap_or(0);
        let bit = (byte >> (7 - self.bitpos % 8)) & 1;
        self.bitpos += 1;
        u32::from(bit)
    }

    fn bits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |acc, _| (acc << 1) | self.bit())
    }
}

/// Adaptive frequency model over a contiguous symbol range.
struct Model {
    first_symbol: u32,
    increment: u32,
    limit: u32,
    total: u32,
    freqs: Vec<u32>,
}

impl Model {
    fn new(first: u32, last: u32, increment: u32, limit: u32) -> Self {
        let mut model = Self {
            first_symbol: first,
            increment,
            limit,
            total: 0,
            freqs: vec![0; (last - first + 1) as usize],
        };
        model.reset();
        model
    }

    fn reset(&mut self) {
        self.freqs.fill(self.increment);
        self.total = self.increment * self.freqs.len() as u32;
    }

    /// Bumps one symbol; once the total passes the limit every frequency
    /// is halved (rounding up) so the model keeps adapting.
    fn increase(&mut self, index: usize) {
        self.freqs[index] += self.increment;
        self.total += self.increment;
        if self.total > self.limit {
            self.total = 0;
            for f in &mut self.freqs {
                *f = (*f + 1) >> 1;
                self.total += *f;
            }
        }
    }
}

const NUM_BITS: u32 = 26;
const ONE: u32 = 1 << (NUM_BITS - 1);
const HALF: u32 = 1 << (NUM_BITS - 2);

/// Range/arithmetic decoder with a 26-bit code window.
struct Decoder<'a> {
    input: BitReader<'a>,
    range: u32,
    code: u32,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let mut input = BitReader { bytes, bitpos: 0 };
        let code = input.bits(NUM_BITS);
        Self {
            input,
            range: ONE,
            code,
        }
    }

    fn symbol(&mut self, model: &mut Model) -> Result<u32, Error> {
        let step = self.range / model.total;
        if step == 0 {
            return Err(Error::Corrupt);
        }
        let target = self.code / step;
        let mut cumulative = 0;
        let mut n = 0;
        while n < model.freqs.len() - 1 {
            if cumulative + model.freqs[n] > target {
                break;
            }
            cumulative += model.freqs[n];
            n += 1;
        }
        self.narrow(cumulative, model.freqs[n], model.total)?;
        model.increase(n);
        Ok(model.first_symbol + n as u32)
    }

    fn narrow(&mut self, low: u32, size: u32, total: u32) -> Result<(), Error> {
        let step = self.range / total;
        let low_incr = step * low;
        self.code = self.code.checked_sub(low_incr).ok_or(Error::Corrupt)?;
        // The top symbol absorbs the rounding slack from the division.
        self.range = if low + size == total {
            self.range - low_incr
        } else {
            size * step
        };
        while self.range <= HALF {
            self.range <<= 1;
            self.code = (self.code << 1) | self.input.bit();
        }
        Ok(())
    }

    /// Reads `bits` binary symbols, least significant first.
    fn bit_string(&mut self, model: &mut Model, bits: u32) -> Result<u32, Error> {
        let mut result = 0;
        for i in 0..bits {
            if self.symbol(model)? != 0 {
                result |= 1 << i;
            }
        }
        Ok(result)
    }
}

struct Mtf {
    table: [u8; 256],
}

impl Mtf {
    fn new() -> Self {
        Self {
            table: std::array::from_fn(|i| i as u8),
        }
    }

    fn decode(&mut self, index: usize) -> u8 {
        let value = self.table[index];
        self.table.copy_within(0..index, 1);
        self.table[0] = value;
        value
    }
}

/// Symbol models shared across blocks. The selector picks between zero-run
/// bits (0/1), MTF index 1 (2), one of seven MTF-index buckets (3..=9), or
/// end-of-block (10).
struct Models {
    initial: Model,
    selector: Model,
    mtf: [Model; 7],
}

impl Models {
    fn new() -> Self {
        Self {
            initial: Model::new(0, 1, 1, 256),
            selector: Model::new(0, 10, 8, 1024),
            mtf: [
                Model::new(2, 3, 8, 1024),
                Model::new(4, 7, 4, 1024),
                Model::new(8, 15, 4, 1024),
                Model::new(16, 31, 4, 1024),
                Model::new(32, 63, 2, 1024),
                Model::new(64, 127, 2, 1024),
                Model::new(128, 255, 1, 1024),
            ],
        }
    }

    fn reset_block_models(&mut self) {
        self.selector.reset();
        for m in &mut self.mtf {
            m.reset();
        }
    }
}

/// One BWT block as read from the stream, before the inverse transform.
struct Block {
    data: Vec<u8>,
    start: usize,
    randomized: bool,
}

/// Decodes the selector/MTF symbol stream for one block.
fn read_block(dec: &mut Decoder, models: &mut Models, block_bits: u32) -> Result<Block, Error> {
    let block_size = 1usize << block_bits;
    let mut mtf = Mtf::new();
    let randomized = dec.symbol(&mut models.initial)? != 0;
    let start = dec.bit_string(&mut models.initial, block_bits)? as usize;
    let mut data = Vec::with_capacity(block_size);

    loop {
        let mut sel = dec.symbol(&mut models.selector)?;
        if sel < 2 {
            // Zero runs are coded in bijective base 2: digit 0 adds
            // 1*state, digit 1 adds 2*state, state doubling each digit.
            let mut state = 1usize;
            let mut count = 0usize;
            while sel < 2 {
                count += state * (sel as usize + 1);
                state *= 2;
                sel = dec.symbol(&mut models.selector)?;
            }
            if data.len() + count > block_size {
                return Err(Error::Corrupt);
            }
            let zero = mtf.decode(0);
            data.extend(std::iter::repeat_n(zero, count));
        }

        let index = match sel {
            10 => break,
            2 => 1,
            _ => dec.symbol(&mut models.mtf[(sel - 3) as usize])? as usize,
        };
        if data.len() >= block_size {
            return Err(Error::Corrupt);
        }
        data.push(mtf.decode(index));
    }

    if start >= data.len() {
        return Err(Error::Corrupt);
    }
    models.reset_block_models();
    Ok(Block {
        data,
        start,
        randomized,
    })
}

/// Builds the inverse-BWT successor table: for every position, the index
/// of the next byte in the original order.
fn inverse_bwt(block: &[u8]) -> Vec<u32> {
    let mut counts = [0u32; 256];
    for &b in block {
        counts[usize::from(b)] += 1;
    }
    let mut next = [0u32; 256];
    let mut total = 0;
    for (slot, &count) in next.iter_mut().zip(&counts) {
        *slot = total;
        total += count;
    }
    let mut transform = vec![0u32; block.len()];
    for (i, &b) in block.iter().enumerate() {
        let slot = &mut next[usize::from(b)];
        transform[*slot as usize] = i as u32;
        *slot += 1;
    }
    transform
}

/// Run-length state: after four equal bytes the next byte is a repeat count.
#[derive(Default)]
struct Rle {
    count: u8,
    last: u8,
}

/// Output sink applying RLE expansion and the running CRC-32.
struct Output {
    bytes: Vec<u8>,
    crc: u32,
}

impl Output {
    fn push(&mut self, byte: u8) {
        self.crc = crc32_update(self.crc, byte);
        self.bytes.push(byte);
    }

    fn feed(&mut self, rle: &mut Rle, byte: u8) {
        if rle.count == 4 {
            rle.count = 0;
            for _ in 0..byte {
                self.push(rle.last);
            }
        } else {
            if byte == rle.last {
                rle.count += 1;
            } else {
                rle.count = 1;
                rle.last = byte;
            }
            self.push(byte);
        }
    }
}

/// Walks the inverse BWT, un-randomizes, and pushes bytes through RLE.
fn unpack_block(block: &Block, out: &mut Output) {
    let transform = inverse_bwt(&block.data);
    let mut rle = Rle::default();
    let mut index = block.start;
    let mut rand_index = 0usize;
    let mut rand_next = usize::from(RANDOMIZATION_TABLE[0]);

    for position in 0..block.data.len() {
        index = transform[index] as usize;
        let mut byte = block.data[index];
        // Randomization flips bit 0 at pseudo-random intervals so BWT does
        // not degenerate on highly repetitive input.
        if block.randomized && rand_next == position {
            byte ^= 1;
            rand_index = (rand_index + 1) & 255;
            rand_next += usize::from(RANDOMIZATION_TABLE[rand_index]);
        }
        out.feed(&mut rle, byte);
    }
}

/// Decompresses a complete Arsenic stream, verifying the embedded CRC-32
/// and the expected output length.
pub fn decompress(input: &[u8], expected_len: usize) -> Result<Vec<u8>, Error> {
    let mut dec = Decoder::new(input);
    let mut models = Models::new();

    let magic = (
        dec.bit_string(&mut models.initial, 8)?,
        dec.bit_string(&mut models.initial, 8)?,
    );
    if magic != (u32::from(b'A'), u32::from(b's')) {
        return Err(Error::Corrupt);
    }
    let block_bits = dec.bit_string(&mut models.initial, 4)? + 9;
    let mut end_of_blocks = dec.symbol(&mut models.initial)? != 0;

    let mut out = Output {
        bytes: Vec::with_capacity(expected_len),
        crc: 0xFFFF_FFFF,
    };
    let mut stored_crc = 0;
    while !end_of_blocks {
        let block = read_block(&mut dec, &mut models, block_bits)?;
        if dec.symbol(&mut models.initial)? != 0 {
            stored_crc = dec.bit_string(&mut models.initial, 32)?;
            end_of_blocks = true;
        }
        unpack_block(&block, &mut out);
        if out.bytes.len() > expected_len {
            return Err(Error::Corrupt);
        }
    }

    if out.bytes.len() != expected_len {
        return Err(Error::Corrupt);
    }
    let actual = !out.crc;
    if actual != stored_crc {
        return Err(Error::Checksum {
            expected: stored_crc,
            actual,
        });
    }
    Ok(out.bytes)
}

/// Gaps between bit flips for randomized blocks (from XADMaster).
const RANDOMIZATION_TABLE: [u16; 256] = [
    0xee, 0x56, 0xf8, 0xc3, 0x9d, 0x9f, 0xae, 0x2c, 0xad, 0xcd, 0x24, 0x9d, 0xa6, 0x101, 0x18,
    0xb9, //
    0xa1, 0x82, 0x75, 0xe9, 0x9f, 0x55, 0x66, 0x6a, 0x86, 0x71, 0xdc, 0x84, 0x56, 0x96, 0x56,
    0xa1, //
    0x84, 0x78, 0xb7, 0x32, 0x6a, 0x3, 0xe3, 0x2, 0x11, 0x101, 0x8, 0x44, 0x83, 0x100, 0x43,
    0xe3, //
    0x1c, 0xf0, 0x86, 0x6a, 0x6b, 0xf, 0x3, 0x2d, 0x86, 0x17, 0x7b, 0x10, 0xf6, 0x80, 0x78,
    0x7a, //
    0xa1, 0xe1, 0xef, 0x8c, 0xf6, 0x87, 0x4b, 0xa7, 0xe2, 0x77, 0xfa, 0xb8, 0x81, 0xee, 0x77,
    0xc0, //
    0x9d, 0x29, 0x20, 0x27, 0x71, 0x12, 0xe0, 0x6b, 0xd1, 0x7c, 0xa, 0x89, 0x7d, 0x87, 0xc4,
    0x101, //
    0xc1, 0x31, 0xaf, 0x38, 0x3, 0x68, 0x1b, 0x76, 0x79, 0x3f, 0xdb, 0xc7, 0x1b, 0x36, 0x7b,
    0xe2, //
    0x63, 0x81, 0xee, 0xc, 0x63, 0x8b, 0x78, 0x38, 0x97, 0x9b, 0xd7, 0x8f, 0xdd, 0xf2, 0xa3,
    0x77, //
    0x8c, 0xc3, 0x39, 0x20, 0xb3, 0x12, 0x11, 0xe, 0x17, 0x42, 0x80, 0x2c, 0xc4, 0x92, 0x59,
    0xc8, //
    0xdb, 0x40, 0x76, 0x64, 0xb4, 0x55, 0x1a, 0x9e, 0xfe, 0x5f, 0x6, 0x3c, 0x41, 0xef, 0xd4,
    0xaa, //
    0x98, 0x29, 0xcd, 0x1f, 0x2, 0xa8, 0x87, 0xd2, 0xa0, 0x93, 0x98, 0xef, 0xc, 0x43, 0xed,
    0x9d, //
    0xc2, 0xeb, 0x81, 0xe9, 0x64, 0x23, 0x68, 0x1e, 0x25, 0x57, 0xde, 0x9a, 0xcf, 0x7f, 0xe5,
    0xba, //
    0x41, 0xea, 0xea, 0x36, 0x1a, 0x28, 0x79, 0x20, 0x5e, 0x18, 0x4e, 0x7c, 0x8e, 0x58, 0x7a,
    0xef, //
    0x91, 0x2, 0x93, 0xbb, 0x56, 0xa1, 0x49, 0x1b, 0x79, 0x92, 0xf3, 0x58, 0x4f, 0x52, 0x9c,
    0x2, //
    0x77, 0xaf, 0x2a, 0x8f, 0x49, 0xd0, 0x99, 0x4d, 0x98, 0x101, 0x60, 0x93, 0x100, 0x75, 0x31,
    0xce, //
    0x49, 0x20, 0x56, 0x57, 0xe2, 0xf5, 0x26, 0x2b, 0x8a, 0xbf, 0xde, 0xd0, 0x83, 0x34, 0xf4, 0x17,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_bwt_roundtrip() {
        // BWT of "banana" with sentinel-free rotation sort: "nnbaaa", start row 3.
        let bwt = b"nnbaaa";
        let transform = inverse_bwt(bwt);
        let mut index = 3;
        let mut out = Vec::new();
        for _ in 0..bwt.len() {
            index = transform[index] as usize;
            out.push(bwt[index]);
        }
        assert_eq!(out, b"banana");
    }

    #[test]
    fn rle_expands_after_four() {
        let mut out = Output {
            bytes: Vec::new(),
            crc: 0xFFFF_FFFF,
        };
        let mut rle = Rle::default();
        for b in [7, 7, 7, 7, 3, 9] {
            out.feed(&mut rle, b);
        }
        assert_eq!(out.bytes, [7, 7, 7, 7, 7, 7, 7, 9]);
    }
}
