//! `snd ` resource decoding (format 1, one `bufferCmd` with a standard
//! 8-bit unsigned mono sound header) and a tiny WAV writer.

use crate::{Error, Reader, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub sample_rate: f64,
    /// Unsigned 8-bit mono samples.
    pub samples: Vec<u8>,
}

const BUFFER_CMD: u16 = 0x8051;
const SOUND_CMD: u16 = 0x8050;

pub fn decode(snd: &[u8]) -> Result<Sound> {
    let mut r = Reader::new(snd);
    let format = r.u16("format")?;
    match format {
        1 => {
            let formats = usize::from(r.u16("data formats")?);
            r.skip(formats * 6, "data formats")?;
        }
        2 => r.skip(2, "reference count")?,
        other => return Err(Error::Unsupported(format!("snd format {other}"))),
    }
    let commands = r.u16("command count")?;
    for _ in 0..commands {
        let cmd = r.u16("command")?;
        r.skip(2, "param1")?;
        let param2 = r.u32("param2")? as usize;
        if cmd == BUFFER_CMD || cmd == SOUND_CMD {
            return sound_header(Reader::at(snd, param2)?);
        }
    }
    Err(Error::Unsupported("snd without a bufferCmd".into()))
}

fn sound_header(mut r: Reader<'_>) -> Result<Sound> {
    r.skip(4, "sample pointer")?;
    let len = r.u32("sample count")? as usize;
    // Fixed-point 16.16 Hz.
    let sample_rate = f64::from(r.u32("sample rate")?) / 65536.0;
    r.skip(4 + 4, "loop points")?;
    let encoding = r.u8("encoding")?;
    if encoding != 0 {
        return Err(Error::Unsupported(format!("snd encoding {encoding:#04x}")));
    }
    r.skip(1, "base frequency")?;
    Ok(Sound {
        sample_rate,
        samples: r.bytes(len, "samples")?.to_vec(),
    })
}

impl Sound {
    /// Encode as an 8-bit PCM mono WAV file.
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = self.samples.len() as u32;
        let rate = self.sample_rate.round() as u32;
        let mut out = Vec::with_capacity(44 + self.samples.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes()); // byte rate: 1 byte per frame
        out.extend_from_slice(&1u16.to_le_bytes()); // block align
        out.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&self.samples);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_format_1_buffer_cmd() {
        let mut snd = vec![0, 1, 0, 1, 0, 5, 0, 0, 0, 0xa0, 0, 1];
        snd.extend_from_slice(&[0x80, 0x51, 0, 0, 0, 0, 0, 20]);
        snd.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 3]);
        snd.extend_from_slice(&(22050u32 << 16).to_be_bytes());
        snd.extend_from_slice(&[0; 8]);
        snd.extend_from_slice(&[0, 60, 128, 200, 50]);
        let s = decode(&snd).unwrap();
        assert_eq!(s.samples, [128, 200, 50]);
        assert!((s.sample_rate - 22050.0).abs() < f64::EPSILON);
        let wav = s.to_wav();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 47);
    }
}
