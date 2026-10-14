//! Descobre os servidores do jogo pela tabela de conexões TCP do sistema
//! operacional (no Windows, `GetExtendedTcpTable`).
//!
//! Isso lê uma tabela pública do SO, igual ao `netstat -ano`. O processo do
//! jogo não é aberto nem tocado.

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddrV4};

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Trecho do nome do executável usado para reconhecer o jogo.
/// Configurável porque o nome no cliente global ainda não foi confirmado.
pub const DEFAULT_PROCESS_HINTS: &[&str] = &["aion2", "aion 2"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameConnection {
    pub pid: u32,
    pub process: String,
    pub local: SocketAddrV4,
    pub remote: SocketAddrV4,
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("não foi possível ler a tabela de conexões: {0}")]
    Netstat(String),
}

/// Lista as conexões TCP IPv4 estabelecidas pelo processo do jogo.
pub fn game_connections(process_hints: &[&str]) -> Result<Vec<GameConnection>, DiscoverError> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let game_pids: Vec<(u32, String)> = sys
        .processes()
        .iter()
        .filter_map(|(pid, p)| {
            let name = p.name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            process_hints
                .iter()
                .any(|h| lower.contains(h))
                .then(|| (pid.as_u32(), name))
        })
        .collect();
    if game_pids.is_empty() {
        return Ok(Vec::new());
    }

    let sockets = get_sockets_info(AddressFamilyFlags::IPV4, ProtocolFlags::TCP)
        .map_err(|e| DiscoverError::Netstat(e.to_string()))?;
    let mut out = Vec::new();
    for s in sockets {
        let ProtocolSocketInfo::Tcp(tcp) = s.protocol_socket_info else {
            continue;
        };
        if tcp.state != TcpState::Established {
            continue;
        }
        let (IpAddr::V4(l), IpAddr::V4(r)) = (tcp.local_addr, tcp.remote_addr) else {
            continue;
        };
        if r.is_loopback() {
            continue;
        }
        for (pid, name) in &game_pids {
            if s.associated_pids.contains(pid) {
                out.push(GameConnection {
                    pid: *pid,
                    process: name.clone(),
                    local: SocketAddrV4::new(l, tcp.local_port),
                    remote: SocketAddrV4::new(r, tcp.remote_port),
                });
            }
        }
    }
    Ok(out)
}

/// Filtro BPF que só deixa passar o que vem dos servidores do jogo.
pub fn bpf_for(servers: &HashSet<SocketAddrV4>) -> String {
    if servers.is_empty() {
        return "tcp".into();
    }
    let mut parts: Vec<String> = servers
        .iter()
        .map(|s| format!("(src host {} and src port {})", s.ip(), s.port()))
        .collect();
    parts.sort();
    format!("tcp and ({})", parts.join(" or "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpf() {
        let mut s = HashSet::new();
        s.insert("1.2.3.4:13328".parse().unwrap());
        assert_eq!(bpf_for(&s), "tcp and ((src host 1.2.3.4 and src port 13328))");
        assert_eq!(bpf_for(&HashSet::new()), "tcp");
    }
}
