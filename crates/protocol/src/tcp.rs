//! Reordenação de segmentos TCP de uma direção da conexão.
//!
//! Portado do `PacketAlignmenter` do TK Aion2-Dps-Meter (MIT), com tratamento
//! de volta do número de sequência (wraparound) e limite de memória.

use std::collections::BTreeMap;

/// Quantos bytes fora de ordem guardamos antes de desistir e ressincronizar.
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub struct TcpAligner {
    next_seq: Option<u32>,
    /// Chave: distância (com wraparound) a partir de `next_seq`.
    pending: BTreeMap<u32, Vec<u8>>,
    pending_bytes: usize,
    /// Quantas vezes perdemos dados e tivemos que pular adiante.
    pub gaps: u64,
}

impl TcpAligner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recebe um segmento e devolve, em ordem, os bytes que já podem ser
    /// entregues ao framing. Retransmissões e sobreposições são cortadas.
    pub fn feed(&mut self, seq: u32, payload: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if payload.is_empty() {
            return out;
        }
        let next = *self.next_seq.get_or_insert(seq);
        let rel = seq.wrapping_sub(next);

        // Segmento todo antigo (retransmissão) ou parcialmente antigo.
        if rel > u32::MAX / 2 {
            let behind = next.wrapping_sub(seq) as usize;
            if behind >= payload.len() {
                return out;
            }
            return self.feed(next, &payload[behind..]);
        }

        if !self.pending.contains_key(&rel) {
            self.pending_bytes += payload.len();
            self.pending.insert(rel, payload.to_vec());
        }

        self.drain(&mut out);

        if self.pending_bytes > MAX_PENDING_BYTES {
            // Buraco que nunca vai ser preenchido: pula para o próximo dado.
            self.gaps += 1;
            if let Some((&first, _)) = self.pending.iter().next() {
                self.rebase(first);
                self.drain(&mut out);
            }
        }
        out
    }

    /// Esquece o estado (ex.: nova conexão).
    pub fn reset(&mut self) {
        *self = Self {
            gaps: self.gaps,
            ..Self::default()
        };
    }

    fn drain(&mut self, out: &mut Vec<Vec<u8>>) {
        while let Some((&rel, _)) = self.pending.iter().next() {
            if rel != 0 {
                break;
            }
            let chunk = self.pending.remove(&0).unwrap();
            self.pending_bytes -= chunk.len();
            self.rebase(chunk.len() as u32);
            out.push(chunk);
        }
    }

    /// Avança `next_seq` em `by` bytes e reindexa o que está pendente,
    /// cortando o que ficou para trás.
    fn rebase(&mut self, by: u32) {
        let next = self.next_seq.unwrap_or(0).wrapping_add(by);
        self.next_seq = Some(next);
        let old = std::mem::take(&mut self.pending);
        self.pending_bytes = 0;
        for (rel, chunk) in old {
            if rel >= by {
                let new_rel = rel - by;
                self.pending_bytes += chunk.len();
                self.pending.entry(new_rel).or_insert(chunk);
            } else {
                let overlap = (by - rel) as usize;
                if overlap < chunk.len() {
                    let rest = chunk[overlap..].to_vec();
                    self.pending_bytes += rest.len();
                    self.pending.entry(0).or_insert(rest);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: Vec<Vec<u8>>) -> Vec<u8> {
        v.concat()
    }

    #[test]
    fn in_order() {
        let mut a = TcpAligner::new();
        assert_eq!(flat(a.feed(100, b"abc")), b"abc");
        assert_eq!(flat(a.feed(103, b"de")), b"de");
    }

    #[test]
    fn out_of_order_is_held() {
        let mut a = TcpAligner::new();
        assert_eq!(flat(a.feed(100, b"abc")), b"abc");
        assert!(a.feed(106, b"ghi").is_empty());
        assert_eq!(flat(a.feed(103, b"def")), b"defghi");
    }

    #[test]
    fn retransmission_and_overlap() {
        let mut a = TcpAligner::new();
        a.feed(100, b"abc");
        assert!(a.feed(100, b"abc").is_empty());
        assert_eq!(flat(a.feed(101, b"bcdef")), b"def");
    }

    #[test]
    fn sequence_wraparound() {
        let mut a = TcpAligner::new();
        let start = u32::MAX - 1;
        assert_eq!(flat(a.feed(start, b"ab")), b"ab");
        assert_eq!(flat(a.feed(0, b"cd")), b"cd");
    }
}
