//! Extrai segmentos TCP de quadros capturados.

use std::net::{Ipv4Addr, SocketAddrV4};

use etherparse::{NetSlice, SlicedPacket, TransportSlice};

/// Tipo de enlace do quadro capturado (o que vem antes do IP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    Ethernet,
    /// Loopback do Npcap/BSD: 4 bytes de família antes do IP.
    Null,
    RawIp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub ts_ms: u64,
    pub src: SocketAddrV4,
    pub dst: SocketAddrV4,
    pub seq: u32,
    pub syn: bool,
    pub fin_or_rst: bool,
    pub payload: Vec<u8>,
}

/// Lê um quadro e devolve o segmento TCP/IPv4, se for um.
pub fn parse_frame(link: LinkType, data: &[u8], ts_ms: u64) -> Option<Segment> {
    let sliced = match link {
        LinkType::Ethernet => SlicedPacket::from_ethernet(data).ok()?,
        LinkType::Null => SlicedPacket::from_ip(data.get(4..)?).ok()?,
        LinkType::RawIp => SlicedPacket::from_ip(data).ok()?,
    };
    let Some(NetSlice::Ipv4(ip)) = &sliced.net else {
        return None;
    };
    let Some(TransportSlice::Tcp(tcp)) = &sliced.transport else {
        return None;
    };
    let h = ip.header();
    let src_ip: Ipv4Addr = h.source_addr();
    let dst_ip: Ipv4Addr = h.destination_addr();
    Some(Segment {
        ts_ms,
        src: SocketAddrV4::new(src_ip, tcp.source_port()),
        dst: SocketAddrV4::new(dst_ip, tcp.destination_port()),
        seq: tcp.sequence_number(),
        syn: tcp.syn(),
        fin_or_rst: tcp.fin() || tcp.rst(),
        payload: tcp.payload().to_vec(),
    })
}

/// Monta um quadro Ethernet/IPv4/TCP (testes e fixtures sintéticas).
pub fn build_ethernet_frame(src: SocketAddrV4, dst: SocketAddrV4, seq: u32, payload: &[u8]) -> Vec<u8> {
    let builder = etherparse::PacketBuilder::ethernet2([2, 0, 0, 0, 0, 1], [2, 0, 0, 0, 0, 2])
        .ipv4(src.ip().octets(), dst.ip().octets(), 64)
        .tcp(src.port(), dst.port(), seq, 65_535)
        .ack(1);
    let mut out = Vec::with_capacity(builder.size(payload.len()));
    builder.write(&mut out, payload).expect("escrita em Vec não falha");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ethernet() {
        let src: SocketAddrV4 = "10.0.0.1:13328".parse().unwrap();
        let dst: SocketAddrV4 = "192.168.0.5:50000".parse().unwrap();
        let f = build_ethernet_frame(src, dst, 77, b"hello");
        let s = parse_frame(LinkType::Ethernet, &f, 5).unwrap();
        assert_eq!(
            (s.src, s.dst, s.seq, s.payload.as_slice()),
            (src, dst, 77, b"hello".as_slice())
        );
    }
}
