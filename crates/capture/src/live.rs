//! Captura ao vivo via Npcap (Windows) ou libpcap (Linux, para desenvolvimento).
//!
//! Abre a interface em modo **não promíscuo** e com filtro BPF: só recebe
//! cópias do tráfego da própria máquina vindo dos servidores do jogo.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Sender, TrySendError};
use pcap::{Capture, Device, Linktype};

use crate::replay::Recorder;
use crate::segment::{parse_frame, LinkType, Segment};

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub description: Option<String>,
    pub ipv4: Vec<Ipv4Addr>,
}

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    #[error("Npcap/libpcap indisponível ou sem permissão: {0}")]
    Pcap(#[from] pcap::Error),
    #[error("nenhuma interface de rede com o IP {0}")]
    NoDevice(Ipv4Addr),
    #[error("tipo de enlace não suportado: {0:?}")]
    UnsupportedLink(Linktype),
    #[error("gravação: {0}")]
    Record(#[from] crate::replay::ReplayError),
}

/// Lista as interfaces. Falha aqui geralmente significa Npcap não instalado.
pub fn list_devices() -> Result<Vec<DeviceInfo>, LiveError> {
    Ok(Device::list()?
        .into_iter()
        .map(|d| DeviceInfo {
            ipv4: d
                .addresses
                .iter()
                .filter_map(|a| match a.addr {
                    std::net::IpAddr::V4(v4) => Some(v4),
                    _ => None,
                })
                .collect(),
            name: d.name,
            description: d.desc,
        })
        .collect())
}

/// Interface que tem o IP local usado na conexão com o jogo.
pub fn device_for_ip(ip: Ipv4Addr) -> Result<String, LiveError> {
    list_devices()?
        .into_iter()
        .find(|d| d.ipv4.contains(&ip))
        .map(|d| d.name)
        .ok_or(LiveError::NoDevice(ip))
}

pub struct LiveConfig {
    pub device: String,
    pub bpf: String,
    pub record_to: Option<PathBuf>,
}

/// Inicia a captura numa thread própria. Para quando `stop` vira `true` ou
/// quando o receptor do canal é descartado. Nunca bloqueia esperando o
/// consumidor: se o canal estiver cheio, o segmento é descartado e contado
/// em `dropped`.
pub fn spawn(
    cfg: LiveConfig,
    tx: Sender<Segment>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
) -> Result<JoinHandle<()>, LiveError> {
    let mut cap = Capture::from_device(cfg.device.as_str())?
        .promisc(false)
        .snaplen(65_535)
        .timeout(200)
        .immediate_mode(true)
        .open()?;
    cap.filter(&cfg.bpf, true)?;
    let link = match cap.get_datalink() {
        Linktype::ETHERNET => LinkType::Ethernet,
        Linktype::NULL | Linktype::LOOP => LinkType::Null,
        Linktype(12) | Linktype(101) => LinkType::RawIp,
        other => return Err(LiveError::UnsupportedLink(other)),
    };
    let mut recorder = match &cfg.record_to {
        Some(p) => Some(Recorder::create(p, link)?),
        None => None,
    };

    Ok(std::thread::Builder::new()
        .name("captura".into())
        .spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match cap.next_packet() {
                    Ok(pkt) => {
                        let ts = now_ms();
                        if let Some(r) = recorder.as_mut() {
                            if let Err(e) = r.write(ts, pkt.data) {
                                tracing::warn!("falha ao gravar pcap: {e}");
                                recorder = None;
                            }
                        }
                        if let Some(seg) = parse_frame(link, pkt.data, ts) {
                            match tx.try_send(seg) {
                                Ok(()) => {}
                                Err(TrySendError::Full(_)) => {
                                    dropped.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(TrySendError::Disconnected(_)) => break,
                            }
                        }
                    }
                    Err(pcap::Error::TimeoutExpired) => continue,
                    Err(e) => {
                        tracing::error!("captura interrompida: {e}");
                        break;
                    }
                }
            }
        })
        .expect("criar thread de captura"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
