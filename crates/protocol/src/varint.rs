//! Inteiros de tamanho variável (LEB128 sem sinal, 7 bits por byte).

/// Valor lido e quantos bytes ele ocupou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarInt {
    pub value: u32,
    pub len: usize,
}

/// Lê um varint em `buf[offset..]`. Retorna `None` se o buffer acabar antes
/// do fim do número ou se ele passar de 32 bits.
pub fn read(buf: &[u8], offset: usize) -> Option<VarInt> {
    let mut value: u32 = 0;
    let mut shift = 0u32;
    let mut count = 0usize;
    loop {
        let byte = *buf.get(offset + count)?;
        count += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(VarInt { value, len: count });
        }
        shift += 7;
        if shift >= 32 {
            return None;
        }
    }
}

/// Codifica um varint (usado nos testes para montar pacotes sintéticos).
pub fn encode(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub(crate) fn u16_le(buf: &[u8], offset: usize) -> Option<u16> {
    let b = buf.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

pub(crate) fn u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let b = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for v in [0u32, 1, 127, 128, 300, 16_383, 16_384, 2_097_151, u32::MAX >> 4] {
            let mut buf = Vec::new();
            encode(v, &mut buf);
            assert_eq!(
                read(&buf, 0),
                Some(VarInt {
                    value: v,
                    len: buf.len()
                })
            );
        }
    }

    #[test]
    fn truncated_is_none() {
        assert_eq!(read(&[0x80, 0x80], 0), None);
        assert_eq!(read(&[], 0), None);
    }

    #[test]
    fn overflow_is_none() {
        assert_eq!(read(&[0xff, 0xff, 0xff, 0xff, 0xff, 0x01], 0), None);
    }
}
