//! CRC routines used by StuffIt: IBM CRC-16 (reflected 0x8005) for stored
//! forks and CRC-32 (reflected 0x04C11DB7) inside Arsenic streams.

/// IBM/ARC CRC-16 with reflected polynomial `0xA001`, initial value 0, no
/// final XOR. Matches XADMaster's `XADCRCTable_a001` unconditioned handle.
pub fn crc16(data: &[u8]) -> u16 {
    data.iter().fold(0u16, |crc, &byte| {
        (0..8).fold(crc ^ u16::from(byte), |c, _| {
            if c & 1 == 0 {
                c >> 1
            } else {
                (c >> 1) ^ 0xA001
            }
        })
    })
}

/// Running CRC-32 (reflected polynomial `0xEDB88320`), updated one byte at a
/// time. Caller supplies the initial `0xFFFF_FFFF` and inverts at the end.
pub fn crc32_update(crc: u32, byte: u8) -> u32 {
    (0..8).fold(crc ^ u32::from(byte), |c, _| {
        if c & 1 == 0 {
            c >> 1
        } else {
            (c >> 1) ^ 0xEDB8_8320
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_check_value() {
        // Standard "123456789" check value for CRC-16/ARC.
        assert_eq!(crc16(b"123456789"), 0xBB3D);
    }

    #[test]
    fn crc32_check_value() {
        let crc = b"123456789"
            .iter()
            .fold(0xFFFF_FFFF, |c, &b| crc32_update(c, b));
        assert_eq!(!crc, 0xCBF4_3926);
    }
}
