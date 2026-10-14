# Aion 2 Assistente: arquitetura e stack

Documento de design, 14/10/2026. Base: relatório de pesquisa do projeto (fontes de dados e riscos), que aprovou a captura passiva de pacotes via Npcap.

## Regra inviolável

O app **só escuta** o tráfego de rede. Ele nunca abre o processo do jogo, nunca lê ou escreve memória, nunca injeta DLL, nunca envia, altera ou bloqueia pacotes. Qualquer funcionalidade que exija quebrar essa regra fica fora do projeto. Essa regra também vale para dependências: nada de WinDivert ou drivers que interceptam em vez de copiar.

## 1. Stack escolhida

| Camada | Escolha | Por quê |
|---|---|---|
| App desktop | **Tauri 2** | Executável pequeno (~10 MB) e pouca RAM, importante rodando ao lado do jogo. Suporta várias janelas, janela transparente e sempre no topo para o overlay. É a stack do medidor aion2t, que já provou o caminho. |
| Backend (captura, parser, cálculos) | **Rust** | Parsing binário seguro e rápido; crate `pcap` sobre Npcap; threads e canais simples. |
| Frontend | **React + TypeScript + Vite** | Ecossistema maior, fácil achar componentes de tabela e gráfico. |
| Banco local | **SQLite** (crate `rusqlite`, migrações com `refinery`) | Arquivo único, sem servidor, fácil de fazer backup. |
| Dados estáticos | JSON versionado no repositório | Catálogo de mascotes, diários/semanais e regras de eventos. Atualizável sem recompilar. |
| Captura | **Npcap** (instalado pelo usuário) | Padrão de todos os medidores. A licença gratuita do Npcap não permite embutir no nosso instalador, então o app detecta e manda o usuário instalar. |
| Instalador e updates | Bundler do Tauri (NSIS) + `tauri-plugin-updater` via GitHub Releases | Instalação em um clique e atualização automática, essencial porque o parser quebra a cada patch. |

**Alternativa considerada:** C# + .NET + WPF com SharpPcap. Também funcionaria bem no Windows, mas o overlay transparente é mais trabalhoso e o executável é maior. Kotlin (como o TK Aion2-Dps-Meter) exige JVM. Por isso Rust + Tauri.

**Referências de código:** o TK Aion2-Dps-Meter é MIT, então podemos portar a lógica de protocolo (opcodes, estrutura dos pacotes) para Rust mantendo o aviso de licença e os créditos. O artigo do aion2t descreve o framing (varints e partes comprimidas).

## 2. Visão geral

```
 Placa de rede ──(cópia passiva)──> Npcap
                                     │
┌──────────────────── Backend Rust ──┼──────────────────────────────┐
│                                    v                              │
│  [capture] thread de captura, filtro BPF no servidor do jogo      │
│      │  pacotes crus (e opcionalmente gravação .pcap)             │
│      v                                                            │
│  [reassembly] remonta o fluxo TCP por conexão                     │
│      │  bytes ordenados                                           │
│      v                                                            │
│  [protocol] framing (varint), descompressão, decodificação        │
│      │  por opcode, tabela de opcodes versionada por patch        │
│      v                                                            │
│  Eventos de domínio: Dano, Cura, Entidade, Grupo, Login, Coleção  │
│      │                                                            │
│      ├──> [combat] lutas, agregação por jogador e skill, DPS      │
│      ├──> [tracker] personagem, mascotes, conteúdo (fase 2)       │
│      └──> [storage] SQLite                                        │
│                                                                   │
│  [scheduler] relógio do servidor, resets e eventos ──> alertas    │
└───────────────────────┬───────────────────────────────────────────┘
                        │ eventos Tauri (snapshot a cada 250 ms)
                        v
        ┌───────────────┴───────────────┐
        │ Janela principal              │ Overlay
        │ histórico, tracker, config    │ DPS ao vivo, banners de alerta
        └───────────────────────────────┘
```

## 3. Módulos

Organizado como workspace Cargo com crates separadas, para que o parser possa ser testado sem a interface.

### 3.1 `capture`
- Lista interfaces do Npcap e escolhe a que carrega o tráfego do jogo.
- **Descobre o servidor do jogo** lendo a tabela de conexões TCP do Windows (`GetExtendedTcpTable`) e filtrando pelo PID do executável do Aion 2. Isso lê uma tabela do sistema operacional, não o processo do jogo. Fallback: IP/porta configuráveis.
- Aplica filtro BPF (`tcp and host <ip> and port <porta>`) para não processar nenhum outro tráfego do usuário.
- **Modo gravação:** salva a sessão em `.pcap` para depuração e testes offline. Desligado por padrão.
- **Modo replay:** lê um `.pcap` gravado em vez da placa. É assim que desenvolvemos e testamos sem estar no jogo.
- Roda numa thread dedicada e envia pacotes por canal (`crossbeam-channel`) para o próximo estágio.

### 3.2 `reassembly`
- Remonta o fluxo TCP por conexão (ordem, retransmissões, buracos). Pode usar a crate `etherparse` para os cabeçalhos.
- Separa direção servidor→cliente (quase todos os dados de combate) da cliente→servidor.
- Ao detectar perda irrecuperável, ressincroniza no próximo início de mensagem válido e registra um contador de erro.

### 3.3 `protocol`
- **Framing:** lê tamanho em varint, separa mensagens, descomprime as partes comprimidas.
- **Decodificadores por opcode** que geram eventos tipados (`DamageEvent`, `HealEvent`, `EntitySpawn`, `EntityDespawn`, `PartyUpdate`, `LocalPlayerInfo`, e futuramente `PetCollection`, `QuestProgress`).
- **Tabela de opcodes versionada:** um arquivo por versão do cliente. Quando um patch muda o protocolo, atualizamos só esse arquivo e os testes.
- Opcodes desconhecidos são contados e, no modo debug, despejados em hex para estudo.
- Esta é a parte mais cara de manter. Ela não depende de Tauri e roda em testes com `.pcap` gravados.

### 3.4 `combat`
- Mantém o **mapa de entidades** (ID → nome, classe, se é jogador, dono de invocação/pet para somar o dano ao dono).
- **Segmentação de lutas:** uma luta começa no primeiro dano e termina após N segundos sem dano (padrão 8 s) ou quando o chefe morre. O usuário também pode resetar por atalho.
- **Agregação:** por jogador e por skill: dano total, DPS, número de golpes, críticos, maior golpe, cura recebida/feita.
- **DPS ao vivo:** dano total / tempo de luta, mais uma média móvel de 5 s para o gráfico.
- Publica um snapshot imutável a cada 250 ms para o frontend, nunca a cada pacote, para não pesar na CPU.
- Ao fim da luta, entrega o resumo ao `storage`.

### 3.5 `storage`
SQLite em `%APPDATA%\AionAssistente\data.db`.

| Tabela | Conteúdo |
|---|---|
| `encounters` | id, início, fim, duração, mapa/zona, nome do alvo principal, dano total, versão do parser |
| `encounter_actors` | luta, nome, classe, dano, DPS, cura, % do total |
| `encounter_skills` | luta, ator, skill, dano, golpes, críticos, maior golpe |
| `characters` | personagens do usuário: nome, servidor, classe, nível (detectado no login ou manual) |
| `pets_catalog` | carregado do JSON estático: 200 mascotes, categoria, origem |
| `character_pets` | personagem, mascote, obtido sim/não, origem do dado (manual ou pacote) |
| `tasks_catalog` | diários e semanais do JSON estático, com tipo de reset |
| `task_completions` | personagem, tarefa, período (data do reset), concluída em |
| `event_rules` | regras de eventos do JSON, por região |
| `alert_settings` | quais eventos alertar, minutos de antecedência, som |
| `settings` | interface de rede, atalhos, opacidade do overlay etc. |

Escrita em thread própria para nunca travar a captura. Retenção de histórico configurável (padrão 90 dias).

### 3.6 `tracker` (fase 2)
- Começa com **marcação manual**: lista de mascotes por categoria com filtro "faltando" e a origem de cada um; checklist de diários e semanais que zera sozinho no reset.
- Depois, se a engenharia reversa achar os pacotes de coleção e progresso no login, o tracker passa a marcar sozinho, e o dado manual vira fallback. A coluna "origem do dado" já prevê isso.

### 3.7 `scheduler` e alertas (fase 3)
- Relógio baseado no **horário do servidor** (fuso da região escolhida), não no fuso do PC.
- Regras declarativas no JSON, por exemplo `{ "id": "fenda", "regiao": "NA", "tipo": "fixo", "horarios": ["01:00","04:00"], "duracao_min": 30 }` ou `{ "tipo": "intervalo", "a_cada_min": 60, "offset_min": 30 }`.
- Calcula o próximo disparo de cada regra e dorme até ele, sem polling.
- Também agenda os resets diário (16:00 horário do servidor global) e semanal (quarta) para zerar o checklist.
- Saídas: notificação do Windows (`tauri-plugin-notification`), banner no overlay e som opcional.

### 3.8 Frontend
**Janela principal**
- Aba Medidor: luta atual e lista de lutas anteriores; clicar abre o detalhe por skill.
- Aba Personagem: mascotes e checklist (fase 2).
- Aba Eventos: próximos eventos com contagem regressiva e liga/desliga de alertas (fase 3).
- Aba Configurações e uma tela de **diagnóstico**: Npcap encontrado, interface, servidor detectado, pacotes/s, opcodes desconhecidos.

**Overlay**
- Janela Tauri separada: sem borda, transparente, sempre no topo, fora da barra de tarefas.
- Mostra barras de DPS do grupo, tempo de luta e banners de alerta.
- **Click-through** por padrão (`set_ignore_cursor_events`); um atalho global alterna para modo de edição para mover e redimensionar. Posição salva nas configurações.
- Requer o jogo em **janela sem borda**. Em tela cheia exclusiva nenhum overlay externo aparece; o app avisa isso na primeira execução.

## 4. Fluxo e threads

1. Thread de captura recebe pacotes do Npcap e envia por canal.
2. Thread de parsing remonta TCP, decodifica e gera eventos de domínio.
3. Thread do motor de combate consome eventos e mantém o estado; a cada 250 ms emite um snapshot com `app.emit("combat:snapshot", ...)`.
4. Thread de storage recebe lutas encerradas e grava em lote.
5. Scheduler roda numa task assíncrona (Tokio) e emite `alert:fire`.
6. O frontend só lê: recebe eventos e chama comandos Tauri (`get_encounters`, `mark_pet`, `reset_encounter` etc.).

Canais com limite de tamanho entre as threads; se o motor ficar para trás, contamos pacotes descartados na tela de diagnóstico em vez de travar.

## 5. Privacidade e segurança

- Tudo fica no PC do usuário. O MVP não envia nada para a internet, exceto a checagem de atualização.
- Nomes de outros jogadores aparecem só localmente. Qualquer upload futuro (ranking, compartilhar luta) será opcional e explícito.
- Arquivos `.pcap` gravados podem conter dados de outros jogadores; a gravação é opt-in e avisa isso.
- O app pede admin só se o Npcap tiver sido instalado com "restringir a administradores". Instruímos a instalar sem essa opção.

## 6. Empacotamento e distribuição

- Instalador NSIS gerado pelo `tauri build`, só Windows 10/11 x64.
- Na primeira execução: checa o Npcap (procura `wpcap.dll` em `System32\Npcap`); se faltar, mostra botão para o site oficial e instruções (marcar "WinPcap API-compatible mode").
- Updater apontando para GitHub Releases, com assinatura do Tauri. Releases rápidas depois de cada patch do jogo.
- Assinatura de código (certificado) é opcional no começo; sem ela o SmartScreen avisa na instalação.
- CI no GitHub Actions: `cargo test` (incluindo replays de `.pcap`), lint do frontend e build do instalador em tag.

## 7. Estrutura do repositório

```
aion2-assistente/
├─ crates/
│  ├─ capture/        Npcap, descoberta do servidor, gravação e replay
│  ├─ protocol/       reassembly, framing, opcodes, eventos
│  ├─ combat/         entidades, lutas, agregação
│  ├─ storage/        SQLite e migrações
│  └─ scheduler/      eventos, resets, alertas
├─ src-tauri/         app Tauri: comandos, janelas, overlay, wiring
├─ ui/                React + TS (janela principal e overlay)
├─ data/              pets.json, tasks.json, events.json, opcodes/<versão>.json
├─ fixtures/          .pcap gravados para testes (sem dados sensíveis)
└─ docs/              este documento, notas de protocolo
```

## 8. Ordem de trabalho do MVP (medidor de DPS)

| # | Etapa | Entrega | Pronto quando |
|---|---|---|---|
| M0 | Esqueleto | Repositório, workspace Cargo, app Tauri abrindo janela, CI | `cargo test` e build passam no CI |
| M1 | Captura e gravação | Detecta Npcap, acha o servidor do jogo, mostra pacotes/s, grava `.pcap` | Você joga 10 min e o arquivo é gravado |
| M2 | Protocolo | Reassembly, framing, descompressão e decodificação de dano/entidades, portando do TK | Replay de um `.pcap` lista os golpes com atacante, alvo, skill e valor |
| M3 | Motor de combate | Lutas, agregação, DPS, invocações somadas ao dono | Testes com fixtures batem com o Combat Analysis oficial (Ctrl+X) em ±2% |
| M4 | Overlay | Barras de DPS ao vivo, click-through, atalhos | Overlay legível sobre o jogo em janela sem borda |
| M5 | Histórico | Gravação de lutas no SQLite, lista e detalhe por skill | Lutas sobrevivem a reiniciar o app |
| M6 | Distribuição | Instalador, checagem do Npcap, updater | Instala num PC limpo e atualiza sozinho |

Depois do MVP:
- **Fase 2, tracker manual:** catálogo de 200 mascotes, checklist com reset automático, vários personagens.
- **Fase 3, alertas:** regras de eventos, contagem regressiva, notificações e banners.
- **Fase 4, tracker automático:** decodificar coleção de mascotes e progresso no login, se o servidor enviar.

O ponto de validação mais importante é o M3: comparar com o Combat Analysis oficial do jogo é o nosso "gabarito" para saber se o parser está certo.

## 9. Riscos técnicos

| Risco | Mitigação |
|---|---|
| Patch muda opcodes ou estrutura | Tabela de opcodes versionada, testes com `.pcap`, updater automático, tela de diagnóstico mostrando opcodes desconhecidos |
| NC adiciona criptografia ao tráfego | Sem solução passiva; nesse caso o plano B é OCR do log de combate. A arquitetura isola isso: um novo módulo geraria os mesmos eventos de domínio |
| Protocolo do servidor global difere do coreano (base do TK) | M2 começa gravando tráfego real do servidor global do usuário antes de portar |
| Licença do Npcap | Não redistribuir; usuário instala pelo site oficial |
| Mudança de posição da NC sobre medidores | Manter a regra só-escuta, acompanhar avisos oficiais, aviso de risco na primeira execução |

## 10. Decisões tomadas (14/10/2026)

- **Nome do app e do repositório:** `aion2-assistente`.
- **Repositório GitHub:** público, `Gimahmoud/aion2-assistente` (a ser criado pelo usuário).
- **Região do servidor:** SA. Resets e eventos usam o horário do servidor SA; os primeiros `.pcap` vêm do servidor SA.
