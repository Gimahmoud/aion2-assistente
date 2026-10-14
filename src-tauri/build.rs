fn main() {
    // O Npcap instala wpcap.dll em System32\Npcap, fora do caminho padrão de
    // DLLs. Carregamos a DLL sob demanda (delay-load) para o app abrir mesmo
    // sem Npcap e poder mostrar a instrução de instalação.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:wpcap.dll");
        println!("cargo:rustc-link-arg-bins=delayimp.lib");
    }
    tauri_build::build()
}
