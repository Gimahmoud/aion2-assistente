# Aion 2 Assistente

Medidor de DPS para Aion 2 no Windows, com overlay sobre o jogo e histórico de lutas. Depois virão o tracker de personagem (mascotes, diários e semanais) e os alertas de eventos.

> Projeto independente, sem vínculo com a NCSoft. O medidor **só escuta** uma cópia do tráfego de rede: não abre o processo do jogo, não lê memória e não envia nem altera pacotes. Mesmo assim, a NCSoft nunca autorizou oficialmente medidores de terceiros. Use por sua conta.

## Estado atual

| Etapa | Situação |
|---|---|
| M0 Esqueleto (workspace, app Tauri, UI, CI) | pronto |
| M1 Captura e gravação (.pcap) | código pronto, **falta testar no Windows com o jogo** |
| M2 Protocolo | portado do TK Aion2-Dps-Meter (servidor coreano, 06/2026). **Falta validar com tráfego real do servidor SA** |
| M3 Motor de combate | pronto com testes; validar contra o Combat Analysis do jogo (Ctrl+X) |
| M4 Overlay | pronto (transparente, clique atravessa, Ctrl+Shift+O para mover) |
| M5 Histórico | pronto (SQLite local) |
| M6 Instalador | build NSIS no CI; updater automático ainda não configurado |

## Como usar (Windows)

1. Instale o [Npcap](https://npcap.com/#download) marcando **WinPcap API-compatible Mode** e deixando desmarcado "Restrict Npcap driver's access to Administrators only".
2. Instale o Aion 2 Assistente pelo instalador da página de Releases (ou do artefato do CI).
3. Deixe o jogo em **janela sem borda**: em tela cheia exclusiva nenhum overlay aparece.
4. Abra o app e entre no jogo. O app encontra sozinho a conexão do Aion 2.

Atalhos: **Ctrl+Shift+R** começa uma nova luta, **Ctrl+Shift+O** destrava o overlay para mover e redimensionar.

### Ajudando a validar o protocolo

Ligue **Gravar tráfego** na janela principal, faça uma luta e compare com o Combat Analysis do jogo (Ctrl+X). As gravações ficam em `%APPDATA%\io.github.gimahmoud.aion2assistente\gravacoes`. A aba **Diagnóstico** mostra mensagens, eventos decodificados e opcodes desconhecidos. Gravações contêm nomes de outros jogadores; não publique sem cuidado.

## Desenvolvimento

Pré-requisitos: Rust estável, Node 22. No Windows, o [Npcap SDK](https://npcap.com/#download) com `LIB` apontando para `Lib\x64`. No Linux, `libpcap-dev` e as [dependências do Tauri](https://tauri.app/start/prerequisites/).

```bash
cargo test                                   # crates de lógica, sem precisar de Tauri
cargo run -p pipeline --bin replay -- fixtures/sintetico-luta-grupo.pcap
cargo run -p pipeline --bin synth -- nova-fixture.pcap

cd ui && npm ci && cd ../src-tauri
../ui/node_modules/.bin/tauri dev            # app com recarga da UI
../ui/node_modules/.bin/tauri build          # instalador
```

O app aceita `--replay arquivo.pcap` para abrir uma gravação na inicialização.

### Estrutura

```
crates/protocol   remontagem TCP, framing varint, LZ4, decodificadores por opcode
crates/combat     lutas, agregação por jogador e skill, DPS
crates/capture    Npcap ao vivo, descoberta do servidor, gravação e replay de .pcap
crates/storage    SQLite (histórico e configurações)
crates/pipeline   liga tudo; CLIs replay e synth
src-tauri         app desktop: thread do medidor, comandos, overlay, atalhos
ui                React + TypeScript + Tailwind + shadcn/ui
data/opcodes      tabela de opcodes por versão do jogo
fixtures          gravações sintéticas para testes (não são tráfego real)
docs              arquitetura
```

A arquitetura completa está em [docs/arquitetura.md](docs/arquitetura.md).

## Créditos

A lógica de protocolo foi portada do [TK Aion2-Dps-Meter](https://github.com/TK-open-public/Aion2-Dps-Meter) (MIT). Veja [NOTICE](NOTICE).

## Licença

MIT
