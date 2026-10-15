//! Separa os segmentos por fluxo (servidor -> cliente) e transforma em
//! mensagens do protocolo.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddrV4;

use protocol::{framing::FramingStats, Message, StreamAssembler, TcpAligner};

use crate::segment::Segment;

/// Como reconhecer que um segmento veio do servidor do jogo.
#[derive(Debug, Clone, Default)]
pub enum ServerFilter {
    /// Endereços conhecidos (descobertos pela tabela de conexões do Windows
    /// ou configurados à mão).
    Endpoints(HashSet<SocketAddrV4>),
    /// Sem informação: assume que o lado com a porta menor é o servidor.
    #[default]
    LowerPort,
}

impl ServerFilter {
    fn is_from_server(&self, s: &Segment) -> bool {
        match self {
            Self::Endpoints(set) => set.contains(&s.src),
            Self::LowerPort => s.src.port() < s.dst.port(),
        }
    }
}

struct Flow {
    aligner: TcpAligner,
    assembler: StreamAssembler,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlowStats {
    pub segments: u64,
    pub bytes: u64,
    pub tcp_gaps: u64,
    pub framing: FramingStats,
}

#[derive(Default)]
pub struct FlowTable {
    filter: ServerFilter,
    flows: HashMap<(SocketAddrV4, SocketAddrV4), Flow>,
    closed_stats: FlowStats,
}

impl FlowTable {
    pub fn new(filter: ServerFilter) -> Self {
        Self {
            filter,
            ..Default::default()
        }
    }

    pub fn set_filter(&mut self, filter: ServerFilter) {
        self.filter = filter;
    }

    /// Recebe um segmento e devolve as mensagens completas que ele liberou.
    pub fn push(&mut self, seg: &Segment) -> Vec<Message> {
        if !self.filter.is_from_server(seg) {
            return Vec::new();
        }
        let key = (seg.src, seg.dst);
        if seg.syn || seg.fin_or_rst {
            if let Some(f) = self.flows.remove(&key) {
                self.absorb(&f);
            }
            if !seg.syn {
                return Vec::new();
            }
        }
        let flow = self.flows.entry(key).or_insert_with(|| Flow {
            aligner: TcpAligner::new(),
            assembler: StreamAssembler::new(),
        });
        self.closed_stats.segments += 1;
        self.closed_stats.bytes += seg.payload.len() as u64;
        // SYN consome um número de sequência; os dados começam em seq+1.
        let seq = if seg.syn { seg.seq.wrapping_add(1) } else { seg.seq };
        let fed = flow.aligner.feed(seq, &seg.payload, seg.ts_ms);
        let mut out = Vec::new();
        for (i, chunk) in fed.chunks.iter().enumerate() {
            if fed.gap_at == Some(i) {
                // Dados perdidos no meio: o que estava pela metade não fecha mais.
                flow.assembler.reset();
            }
            out.extend(flow.assembler.push(chunk));
        }
        out
    }

    pub fn stats(&self) -> FlowStats {
        let mut s = self.closed_stats;
        for f in self.flows.values() {
            s.tcp_gaps += f.aligner.gaps;
            add(&mut s.framing, &f.assembler.stats);
        }
        s
    }

    fn absorb(&mut self, f: &Flow) {
        self.closed_stats.tcp_gaps += f.aligner.gaps;
        add(&mut self.closed_stats.framing, &f.assembler.stats);
    }
}

fn add(a: &mut FramingStats, b: &FramingStats) {
    a.messages += b.messages;
    a.compressed_batches += b.compressed_batches;
    a.resyncs += b.resyncs;
    a.decompress_errors += b.decompress_errors;
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::framing::build_frame;

    fn seg(src: &str, dst: &str, seq: u32, payload: Vec<u8>) -> Segment {
        Segment {
            ts_ms: 0,
            src: src.parse().unwrap(),
            dst: dst.parse().unwrap(),
            seq,
            syn: false,
            fin_or_rst: false,
            payload,
        }
    }

    #[test]
    fn only_server_direction_is_parsed() {
        let mut t = FlowTable::new(ServerFilter::LowerPort);
        let f = build_frame(None, &[0x04, 0x38, 1, 2, 3]);
        assert_eq!(t.push(&seg("1.1.1.1:13328", "2.2.2.2:50000", 10, f.clone())).len(), 1);
        assert!(t.push(&seg("2.2.2.2:50000", "1.1.1.1:13328", 10, f)).is_empty());
    }

    #[test]
    fn message_split_across_segments() {
        let mut t = FlowTable::new(ServerFilter::LowerPort);
        let f = build_frame(None, &[0x04, 0x38, 1, 2, 3, 4, 5, 6]);
        let (a, b) = f.split_at(4);
        assert!(t
            .push(&seg("1.1.1.1:13328", "2.2.2.2:50000", 100, a.to_vec()))
            .is_empty());
        assert_eq!(t.push(&seg("1.1.1.1:13328", "2.2.2.2:50000", 104, b.to_vec())).len(), 1);
    }
}
