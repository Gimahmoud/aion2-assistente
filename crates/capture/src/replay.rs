//! Leitura e gravação de arquivos .pcap.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use pcap_file::pcap::{PcapHeader, PcapPacket, PcapReader, PcapWriter};
use pcap_file::DataLink;

use crate::segment::{parse_frame, LinkType, Segment};

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("erro de arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("pcap inválido: {0}")]
    Pcap(#[from] pcap_file::PcapError),
    #[error("tipo de enlace não suportado: {0:?}")]
    UnsupportedLink(DataLink),
}

pub fn link_from_datalink(d: DataLink) -> Result<LinkType, ReplayError> {
    match d {
        DataLink::ETHERNET => Ok(LinkType::Ethernet),
        DataLink::NULL | DataLink::LOOP => Ok(LinkType::Null),
        DataLink::RAW | DataLink::IPV4 => Ok(LinkType::RawIp),
        other => Err(ReplayError::UnsupportedLink(other)),
    }
}

/// Lê todos os segmentos TCP/IPv4 de um .pcap, na ordem do arquivo.
///
/// Uma gravação copiada enquanto o app ainda grava termina com um pacote
/// pela metade: nesse caso fica tudo o que veio antes dele.
pub fn read_segments(path: &Path) -> Result<Vec<Segment>, ReplayError> {
    let mut reader = PcapReader::new(BufReader::new(File::open(path)?))?;
    let link = link_from_datalink(reader.header().datalink)?;
    let mut out = Vec::new();
    while let Some(pkt) = reader.next_packet() {
        let pkt = match pkt {
            Ok(p) => p,
            Err(_) if !out.is_empty() => break,
            Err(e) => return Err(e.into()),
        };
        let ts = pkt.timestamp.as_millis() as u64;
        if let Some(s) = parse_frame(link, &pkt.data, ts) {
            out.push(s);
        }
    }
    Ok(out)
}

/// Grava quadros crus em .pcap para depuração e testes.
pub struct Recorder {
    writer: PcapWriter<File>,
}

impl Recorder {
    pub fn create(path: &Path, link: LinkType) -> Result<Self, ReplayError> {
        let datalink = match link {
            LinkType::Ethernet => DataLink::ETHERNET,
            LinkType::Null => DataLink::NULL,
            LinkType::RawIp => DataLink::RAW,
        };
        let header = PcapHeader {
            datalink,
            ..Default::default()
        };
        Ok(Self {
            writer: PcapWriter::with_header(File::create(path)?, header)?,
        })
    }

    pub fn write(&mut self, ts_ms: u64, frame: &[u8]) -> Result<(), ReplayError> {
        let ts = std::time::Duration::from_millis(ts_ms);
        self.writer
            .write_packet(&PcapPacket::new(ts, frame.len() as u32, frame))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::build_ethernet_frame;

    #[test]
    fn record_then_replay() {
        let dir = std::env::temp_dir().join(format!("aion2-replay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.pcap");
        let src = "10.0.0.1:13328".parse().unwrap();
        let dst = "10.0.0.2:50000".parse().unwrap();
        {
            let mut r = Recorder::create(&path, LinkType::Ethernet).unwrap();
            r.write(1_000, &build_ethernet_frame(src, dst, 1, b"abc")).unwrap();
            r.write(2_000, &build_ethernet_frame(src, dst, 4, b"def")).unwrap();
        }
        let segs = read_segments(&path).unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].ts_ms, 2_000);
        assert_eq!(segs[1].payload, b"def");
        std::fs::remove_dir_all(dir).ok();
    }
}
