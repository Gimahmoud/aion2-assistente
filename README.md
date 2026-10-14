# Aion 2 Assistente

Medidor de DPS para Aion 2 no Windows, com overlay sobre o jogo e histórico de lutas, mais um tracker de personagem (mascotes, diárias e semanais) e alertas de eventos.

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
| Tracker de personagem | pronto, com marcação manual |
| Alertas de eventos | pronto; horários do servidor global ainda a conferir no jogo |

## Como usar (Windows)

1. Instale o [Npcap](https://npcap.com/#download) marcando **WinPcap API-compatible Mode** e deixando desmarcado "Restrict Npcap driver's access to Administrators only".
2. Instale o Aion 2 Assistente pelo instalador da página de Releases (ou do artefato do CI).
3. Deixe o jogo em **janela sem borda**: em tela cheia exclusiva nenhum overlay aparece.
4. Abra o app e entre no jogo. O app encontra sozinho a conexão do Aion 2.

Atalhos: **Ctrl+Shift+R** começa uma nova luta, **Ctrl+Shift+O** destrava o overlay para mover e redimensionar.

### Personagens: mascotes, diárias e semanais

Na aba **Personagens**, adicione cada personagem seu. Para cada um:

- **Diárias e semanais**: marque o que já fez. Tarefas com limite (por exemplo 5 Missões de dever) têm um contador. Tudo zera sozinho no reset do servidor, e o app mostra quanto falta para o próximo. O selo **servidor** indica que o limite é dividido entre os personagens do mesmo servidor. Dá para criar tarefas suas no fim da página.
- **Mascotes**: marque os que você já tem e use o filtro **Faltando** para ver o que ainda falta. A lista que vem com o app ainda é parcial; adicione os que faltarem pelo formulário no fim da página.

### Eventos e alertas

A aba **Eventos** mostra o que acontece nas próximas 24 horas no horário do seu computador (Fenda Espaço-Temporal, chefes do Abismo, resets e outros). Em **Alertas**, ligue os eventos que quer acompanhar e escolha com quantos minutos de antecedência ser avisado. O aviso chega como notificação do Windows enquanto o app estiver aberto, mesmo minimizado. Use **Testar notificação** para conferir se o Windows está deixando o app avisar.

Os horários seguem o calendário fixo do servidor global (reset diário às 04:00 de Brasília, semanal na quarta). Se algum horário não bater com o jogo, avise.

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
crates/tracker    personagens, mascotes, diárias/semanais, agenda de eventos e alertas
src-tauri         app desktop: thread do medidor, comandos, overlay, atalhos
ui                React + TypeScript + Tailwind + shadcn/ui
data/opcodes      tabela de opcodes por versão do jogo
data/tracker      mascotes, tarefas, eventos e relógio do servidor (JSON)
fixtures          gravações sintéticas para testes (não são tráfego real)
docs              arquitetura
```

A arquitetura completa está em [docs/arquitetura.md](docs/arquitetura.md).

## Créditos

A lógica de protocolo foi portada do [TK Aion2-Dps-Meter](https://github.com/TK-open-public/Aion2-Dps-Meter) (MIT). Veja [NOTICE](NOTICE).

## Licença

MIT
