//! Reordenação de segmentos TCP de uma direção da conexão.
//!
//! Portado do `PacketAlignmenter` do TK Aion2-Dps-Meter (MIT), com tratamento
//! de volta do número de sequência (wraparound) e limite de memória.

use std::collections::BTreeMap;

/// Quantos bytes fora de ordem guardamos antes de desistir do buraco.
const MAX_PENDING_BYTES: usize = 512 * 1024;
/// Quanto tempo esperamos uma retransmissão antes de pular o buraco. Se o
/// Npcap perdeu o pacote, ele nunca chega; esperar para sempre trava a leitura.
const GAP_TIMEOUT_MS: u64 = 1_500;

/// Resultado de [`TcpAligner::feed`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Fed {
    /// Bytes em ordem, prontos para o framing.
    pub chunks: Vec<Vec<u8>>,
    /// Se houve um buraco, os chunks a partir deste índice vêm depois dele e
    /// o framing precisa recomeçar do zero.
    pub gap_at: Option<usize>,
}

#[derive(Default)]
pub struct TcpAligner {
    next_seq: Option<u32>,
    /// Chave: distância (com wraparound) a partir de `next_seq`.
    pending: BTreeMap<u32, Vec<u8>>,
    pending_bytes: usize,
    /// Desde quando (ms) há dados esperando um buraco ser preenchido.
    waiting_since: Option<u64>,
    /// Quantas vezes perdemos dados e tivemos que pular adiante.
    pub gaps: u64,
}

impl TcpAligner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recebe um segmento e devolve, em ordem, os bytes que já podem ser
    /// entregues ao framing. Retransmissões e sobreposições são cortadas.
    pub fn feed(&mut self, seq: u32, payload: &[u8], ts_ms: u64) -> Fed {
        let mut fed = Fed::default();
        if !payload.is_empty() {
            self.insert(seq, payload);
        }
        self.drain(&mut fed.chunks);

        if self.pending.is_empty() {
            self.waiting_since = None;
        } else {
            let since = *self.waiting_since.get_or_insert(ts_ms);
            let stale = ts_ms.saturating_sub(since) > GAP_TIMEOUT_MS;
            if stale || self.pending_bytes > MAX_PENDING_BYTES {
                // Buraco que não vai ser preenchido: pula para o próximo dado.
                self.gaps += 1;
                fed.gap_at = Some(fed.chunks.len());
                if let Some((&first, _)) = self.pending.iter().next() {
                    self.rebase(first);
                    self.drain(&mut fed.chunks);
                }
                self.waiting_since = (!self.pending.is_empty()).then_some(ts_ms);
            }
        }
        fed
    }

    fn insert(&mut self, seq: u32, payload: &[u8]) {
        let next = *self.next_seq.get_or_insert(seq);
        let rel = seq.wrapping_sub(next);

        // Segmento todo antigo (retransmissão) ou parcialmente antigo.
        if rel > u32::MAX / 2 {
            let behind = next.wrapping_sub(seq) as usize;
            if behind < payload.len() {
                self.insert(next, &payload[behind..]);
            }
            return;
        }

        if !self.pending.contains_key(&rel) {
            self.pending_bytes += payload.len();
            self.pending.insert(rel, payload.to_vec());
        }
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

    fn flat(f: Fed) -> Vec<u8> {
        f.chunks.concat()
    }

    #[test]
    fn in_order() {
        let mut a = TcpAligner::new();
        assert_eq!(flat(a.feed(100, b"abc", 0)), b"abc");
        assert_eq!(flat(a.feed(103, b"de", 0)), b"de");
    }

    #[test]
    fn out_of_order_is_held() {
        let mut a = TcpAligner::new();
        assert_eq!(flat(a.feed(100, b"abc", 0)), b"abc");
        assert!(a.feed(106, b"ghi", 0).chunks.is_empty());
        assert_eq!(flat(a.feed(103, b"def", 0)), b"defghi");
    }

    #[test]
    fn retransmission_and_overlap() {
        let mut a = TcpAligner::new();
        a.feed(100, b"abc", 0);
        assert!(a.feed(100, b"abc", 0).chunks.is_empty());
        assert_eq!(flat(a.feed(101, b"bcdef", 0)), b"def");
    }

    #[test]
    fn sequence_wraparound() {
        let mut a = TcpAligner::new();
        let start = u32::MAX - 1;
        assert_eq!(flat(a.feed(start, b"ab", 0)), b"ab");
        assert_eq!(flat(a.feed(0, b"cd", 0)), b"cd");
    }

    #[test]
    fn lost_segment_is_skipped_after_timeout() {
        let mut a = TcpAligner::new();
        a.feed(100, b"abc", 0);
        // 103..106 perdido pelo Npcap; nunca chega.
        assert!(a.feed(106, b"ghi", 100).chunks.is_empty());
        assert!(a.feed(109, b"jk", 1_000).chunks.is_empty());
        let fed = a.feed(111, b"l", 2_000);
        assert_eq!(fed.gap_at, Some(0));
        assert_eq!(flat(fed), b"ghijkl");
        assert_eq!(a.gaps, 1);
        // Depois do pulo, segue normal.
        assert_eq!(flat(a.feed(112, b"m", 2_100)), b"m");
    }
}
