//! Detecção do Npcap no Windows.

/// Pasta onde o Npcap instala `wpcap.dll` e `Packet.dll`.
#[cfg(windows)]
fn npcap_dir() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    std::path::Path::new(&root).join("System32").join("Npcap")
}

/// Verifica se o Npcap está instalado e, se estiver, adiciona a pasta dele
/// ao caminho de busca de DLLs. Precisa rodar antes de qualquer chamada ao
/// pcap, porque `wpcap.dll` é carregada sob demanda.
#[cfg(windows)]
pub fn prepare() -> bool {
    use std::os::windows::ffi::OsStrExt;
    let dir = npcap_dir();
    if !dir.join("wpcap.dll").exists() {
        return false;
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: string terminada em zero, válida durante a chamada.
    unsafe { windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW(wide.as_ptr()) != 0 }
}

/// Fora do Windows usamos a libpcap do sistema (só para desenvolvimento).
#[cfg(not(windows))]
pub fn prepare() -> bool {
    true
}

pub const DOWNLOAD_URL: &str = "https://npcap.com/#download";
