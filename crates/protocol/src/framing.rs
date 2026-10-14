//! Separa o fluxo TCP em mensagens e abre os pacotes comprimidos.
//!
//! Formato (portado do TK Aion2-Dps-Meter, MIT, versão coreana de 06/2026):
//!
//! ```text
//! [varint tamanho][flag extra opcional 0xF0..0xFE][corpo...]
//! tamanho real da mensagem = valor do varint + bytes do varint - 4
//! corpo começando com FF FF = lote comprimido:
//!     FF FF [u32 LE tamanho original][bloco LZ4 com várias mensagens]
//! senão, os 2 primeiros bytes do corpo são o opcode.
//! ```

use crate::varint;

/// Buffer acima disso indica que perdemos o alinhamento; descartamos.
const MAX_BUFFER: usize = 2 * 1024 * 1024;
/// Teto para o tamanho descomprimido, contra lixo no cabeçalho.
const MAX_DECOMPRESSED: usize = 8 * 1024 * 1024;

/// Uma mensagem completa, já fora do lote comprimido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub bytes: Vec<u8>,
    /// Tamanho do varint de cabeçalho.
    pub header_len: usize,
    /// Se há o byte extra (0xF0..0xFE) logo após o cabeçalho.
    pub extra_flag: bool,
}

impl Message {
    /// Posição do opcode dentro de `bytes`.
    pub fn opcode_offset(&self) -> usize {
        self.header_len + usize::from(self.extra_flag)
    }

    /// Opcode como `b1 | b2 << 8`, igual à tabela de opcodes.
    pub fn opcode(&self) -> Option<u16> {
        varint::u16_le(&self.bytes, self.opcode_offset())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FramingStats {
    pub messages: u64,
    pub compressed_batches: u64,
    pub resyncs: u64,
    pub decompress_errors: u64,
}

/// Acumula bytes de uma direção do fluxo e devolve mensagens completas.
#[derive(Default)]
pub struct StreamAssembler {
    buf: Vec<u8>,
    pub stats: FramingStats,
}

impl StreamAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Message> {
        if self.buf.len() + chunk.len() > MAX_BUFFER {
            self.stats.resyncs += 1;
            self.buf.clear();
            return Vec::new();
        }
        self.buf.extend_from_slice(chunk);

        let mut out = Vec::new();
        let mut pos = 0usize;
        while pos < self.buf.len() {
            let Some(len) = varint::read(&self.buf, pos) else {
                // Varint incompleto (espera mais dados) ou inválido.
                if self.buf.len() - pos >= 5 {
                    self.stats.resyncs += 1;
                    pos = self.buf.len();
                }
                break;
            };
            if len.value == 0 {
                pos += 1;
                continue;
            }
            let real = (len.value as i64) + (len.len as i64) - 4;
            if real <= 0 {
                self.stats.resyncs += 1;
                pos = self.buf.len();
                break;
            }
            let real = real as usize;
            if self.buf.len() - pos < real {
                break;
            }
            let frame = self.buf[pos..pos + real].to_vec();
            pos += real;
            split_message(frame, &mut out, &mut self.stats);
        }
        self.buf.drain(..pos);
        out
    }
}

/// Classifica um frame: mensagem simples, lote comprimido ou descartável.
fn split_message(frame: Vec<u8>, out: &mut Vec<Message>, stats: &mut FramingStats) {
    if frame.len() <= 3 {
        return;
    }
    let Some(len) = varint::read(&frame, 0) else { return };
    let h = len.len;
    let Some(&first) = frame.get(h) else { return };
    let extra_flag = (0xf0..0xff).contains(&first);
    let body = h + usize::from(extra_flag);

    if frame.get(body) == Some(&0xff) && frame.get(body + 1) == Some(&0xff) {
        stats.compressed_batches += 1;
        if let Err(()) = decompress_batch(&frame, body + 2, out, stats) {
            stats.decompress_errors += 1;
        }
        return;
    }

    stats.messages += 1;
    out.push(Message {
        bytes: frame,
        header_len: h,
        extra_flag,
    });
}

fn decompress_batch(frame: &[u8], offset: usize, out: &mut Vec<Message>, stats: &mut FramingStats) -> Result<(), ()> {
    let original = varint::u32_le(frame, offset).ok_or(())? as usize;
    if original == 0 || original > MAX_DECOMPRESSED {
        return Err(());
    }
    let mut restored = vec![0u8; original];
    let n = lz4_flex::block::decompress_into(&frame[offset + 4..], &mut restored).map_err(|_| ())?;
    restored.truncate(n);

    let mut pos = 0usize;
    while pos < restored.len() {
        let len = varint::read(&restored, pos).ok_or(())?;
        if len.value == 0 {
            pos += 1;
            continue;
        }
        let real = (len.value as i64) + (len.len as i64) - 4;
        if real <= 0 || pos + real as usize > restored.len() {
            return Err(());
        }
        let inner = restored[pos..pos + real as usize].to_vec();
        pos += real as usize;
        split_message(inner, out, stats);
    }
    Ok(())
}

/// Monta um frame com o cabeçalho correto (para testes e fixtures sintéticas).
pub fn build_frame(extra_flag: Option<u8>, body: &[u8]) -> Vec<u8> {
    let content = usize::from(extra_flag.is_some()) + body.len();
    // real = valor + len_varint - 4  =>  valor = real - len_varint + 4
    for vlen in 1..=5usize {
        let real = vlen + content;
        let value = (real + 4 - vlen) as u32;
        let mut hdr = Vec::new();
        varint::encode(value, &mut hdr);
        if hdr.len() == vlen {
            let mut f = hdr;
            if let Some(x) = extra_flag {
                f.push(x);
            }
            f.extend_from_slice(body);
            return f;
        }
    }
    unreachable!("corpo grande demais")
}

/// Monta um lote comprimido contendo os frames dados.
pub fn build_compressed_frame(frames: &[Vec<u8>]) -> Vec<u8> {
    let inner = frames.concat();
    let compressed = lz4_flex::block::compress(&inner);
    let mut body = vec![0xff, 0xff];
    body.extend_from_slice(&(inner.len() as u32).to_le_bytes());
    body.extend_from_slice(&compressed);
    build_frame(None, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frames_across_chunks() {
        let a = build_frame(None, &[0x04, 0x38, 1, 2, 3]);
        let b = build_frame(Some(0xf2), &[0x05, 0x38, 9, 9]);
        let stream = [a.clone(), b.clone()].concat();
        let mut asm = StreamAssembler::new();
        let mut got = asm.push(&stream[..4]);
        got.extend(asm.push(&stream[4..]));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].bytes, a);
        assert_eq!(got[0].opcode(), Some(0x3804));
        assert!(got[1].extra_flag);
        assert_eq!(got[1].opcode(), Some(0x3805));
    }

    #[test]
    fn opens_compressed_batch() {
        let inner = vec![
            build_frame(None, &[0x04, 0x38, 1, 2, 3, 4]),
            build_frame(None, &[0x33, 0x36, 7, 7, 7]),
        ];
        let frame = build_compressed_frame(&inner);
        let mut asm = StreamAssembler::new();
        let got = asm.push(&frame);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].bytes, inner[0]);
        assert_eq!(got[1].opcode(), Some(0x3633));
        assert_eq!(asm.stats.compressed_batches, 1);
    }

    #[test]
    fn skips_zero_padding() {
        let a = build_frame(None, &[0x04, 0x38, 1, 2]);
        let mut asm = StreamAssembler::new();
        let got = asm.push(&[[0u8, 0].as_slice(), &a].concat());
        assert_eq!(got.len(), 1);
    }
}
