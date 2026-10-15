//! Thread do medidor: recebe segmentos da captura (ao vivo ou replay),
//! alimenta o pipeline, salva lutas encerradas e emite eventos para a UI.

use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use capture::{discover, live, replay, Segment, ServerFilter};
use combat::{EncounterSnapshot, EngineConfig};
use crossbeam_channel::{bounded, select, tick, Receiver, Sender};
use pipeline::{Pipeline, PipelineStats};
use protocol::OpcodeTable;
use serde::Serialize;
use storage::Store;
use tauri::{AppHandle, Emitter};

const SNAPSHOT_EVERY: Duration = Duration::from_millis(250);
const STATUS_EVERY: Duration = Duration::from_secs(1);
const DISCOVER_EVERY: Duration = Duration::from_secs(5);

pub enum Command {
    Reset,
    Replay(PathBuf),
    SetRecording(Option<PathBuf>),
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    #[default]
    Starting,
    NpcapMissing,
    WaitingForGame,
    Capturing,
    Replaying,
    Error,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    pub state: CaptureState,
    pub message: String,
    pub servers: Vec<String>,
    pub device: Option<String>,
    pub recording: Option<String>,
    pub npcap_download: &'static str,
    pub stats: PipelineStats,
    pub dropped_segments: u64,
    /// Personagem logado no jogo, se o app já viu o login.
    pub own_character: Option<pipeline::OwnCharacter>,
}

pub struct Runtime {
    pub cmd_tx: Sender<Command>,
    pub status: Arc<Mutex<Status>>,
}

struct LiveCapture {
    stop: Arc<AtomicBool>,
    ports: HashSet<u16>,
}

impl Drop for LiveCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn spawn(app: AppHandle, store: Arc<Mutex<Store>>, npcap_ok: bool) -> Runtime {
    let (cmd_tx, cmd_rx) = bounded::<Command>(16);
    let status = Arc::new(Mutex::new(Status {
        npcap_download: crate::npcap::DOWNLOAD_URL,
        ..Default::default()
    }));
    let st = status.clone();
    std::thread::Builder::new()
        .name("medidor".into())
        .spawn(move || Worker::new(app, store, st, npcap_ok).run(cmd_rx))
        .expect("criar thread do medidor");
    Runtime { cmd_tx, status }
}

struct Worker {
    app: AppHandle,
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<Status>>,
    npcap_ok: bool,
    pipeline: Pipeline,
    live: Option<LiveCapture>,
    seg_tx: Sender<Segment>,
    seg_rx: Receiver<Segment>,
    recording: Option<live::Recording>,
    dropped: Arc<AtomicU64>,
}

impl Worker {
    fn new(app: AppHandle, store: Arc<Mutex<Store>>, status: Arc<Mutex<Status>>, npcap_ok: bool) -> Self {
        // Canal limitado: se o motor atrasar, a captura descarta em vez de
        // acumular memória sem fim.
        let (seg_tx, seg_rx) = bounded(20_000);
        Self {
            app,
            store,
            status,
            npcap_ok,
            pipeline: new_pipeline(ServerFilter::LowerPort),
            live: None,
            seg_tx,
            seg_rx,
            recording: None,
            dropped: Default::default(),
        }
    }

    fn run(mut self, cmd_rx: Receiver<Command>) {
        let snap_tick = tick(SNAPSHOT_EVERY);
        let status_tick = tick(STATUS_EVERY);
        let discover_tick = tick(DISCOVER_EVERY);
        self.discover();
        loop {
            select! {
                recv(self.seg_rx) -> seg => {
                    let Ok(seg) = seg else { break };
                    let closed = self.pipeline.push_segment(&seg);
                    self.save(closed);
                }
                recv(cmd_rx) -> cmd => match cmd {
                    Ok(Command::Reset) => {
                        let closed = self.pipeline.reset();
                        self.save(closed);
                        self.emit_snapshot();
                    }
                    Ok(Command::Replay(path)) => self.replay(path),
                    Ok(Command::SetRecording(path)) => {
                        self.recording = path.map(live::Recording::new);
                        // Reabre a captura para começar/parar a gravação.
                        self.live = None;
                        self.discover();
                    }
                    Err(_) => break,
                },
                recv(snap_tick) -> _ => {
                    let closed = self.pipeline.tick(now_ms());
                    self.save(closed);
                    self.emit_snapshot();
                }
                recv(status_tick) -> _ => self.emit_status(),
                recv(discover_tick) -> _ => self.discover(),
            }
        }
    }

    /// Procura as conexões do jogo e (re)abre a captura se elas mudaram.
    fn discover(&mut self) {
        if !self.npcap_ok {
            self.set_state(
                CaptureState::NpcapMissing,
                "Npcap não encontrado. Instale pelo site oficial e reinicie o app.",
            );
            return;
        }
        if matches!(self.status.lock().unwrap().state, CaptureState::Replaying) {
            return;
        }
        let conns = match discover::game_connections(discover::DEFAULT_PROCESS_HINTS) {
            Ok(c) => c,
            Err(e) => {
                self.set_state(CaptureState::Error, &e.to_string());
                return;
            }
        };
        if conns.is_empty() {
            self.live = None;
            self.set_state(
                CaptureState::WaitingForGame,
                "Aguardando o Aion 2. Abra o jogo e entre com um personagem.",
            );
            return;
        }
        let servers: HashSet<SocketAddrV4> = conns.iter().map(|c| c.remote).collect();
        let ports = discover::server_ports(&servers);
        self.status.lock().unwrap().servers = servers.iter().map(|s| s.to_string()).collect();
        // A captura filtra pela porta: servidor novo na mesma porta já está
        // sendo capturado, não precisa reabrir.
        if self.live.as_ref().is_some_and(|l| l.ports == ports) {
            return;
        }
        self.live = None;
        let local_ip = *conns[0].local.ip();
        let result = live::device_for_ip(local_ip).and_then(|device| {
            let stop = Arc::new(AtomicBool::new(false));
            let cfg = live::LiveConfig {
                device: device.clone(),
                bpf: discover::bpf_for(&ports),
                recording: self.recording.clone(),
            };
            live::spawn(cfg, self.seg_tx.clone(), stop.clone(), self.dropped.clone())?;
            Ok((device, stop))
        });
        match result {
            Ok((device, stop)) => {
                self.pipeline.set_filter(ServerFilter::Ports(ports.clone()));
                {
                    let mut s = self.status.lock().unwrap();
                    s.device = Some(device);
                    s.recording = self.recording.as_ref().map(|r| r.path.display().to_string());
                }
                self.set_state(
                    CaptureState::Capturing,
                    "Capturando o tráfego do jogo (somente leitura).",
                );
                self.live = Some(LiveCapture { stop, ports });
            }
            Err(e) => self.set_state(CaptureState::Error, &e.to_string()),
        }
    }

    /// Processa um .pcap inteiro com os horários gravados.
    fn replay(&mut self, path: PathBuf) {
        self.live = None;
        self.set_state(CaptureState::Replaying, &format!("Reproduzindo {}", path.display()));
        self.pipeline = new_pipeline(ServerFilter::LowerPort);
        match replay::read_segments(&path) {
            Ok(segs) => {
                for s in &segs {
                    let closed = self.pipeline.push_segment(s);
                    self.save(closed);
                }
                let closed = self.pipeline.reset();
                self.save(closed);
                self.emit_status();
                self.set_state(
                    CaptureState::Starting,
                    &format!("Replay concluído: {} segmentos.", segs.len()),
                );
            }
            Err(e) => self.set_state(CaptureState::Error, &e.to_string()),
        }
        self.pipeline = new_pipeline(ServerFilter::LowerPort);
        self.discover();
    }

    fn save(&self, closed: impl IntoIterator<Item = EncounterSnapshot>) {
        for e in closed {
            if e.duration_ms < 1_000 && e.actors.len() <= 1 && e.total_damage == 0 {
                continue;
            }
            let id = self
                .store
                .lock()
                .unwrap()
                .save_encounter(&e, self.pipeline.opcode_version());
            match id {
                Ok(id) => {
                    let _ = self
                        .app
                        .emit("combat:closed", serde_json::json!({ "db_id": id, "encounter": e }));
                }
                Err(err) => tracing::error!("falha ao salvar luta: {err}"),
            }
        }
    }

    fn emit_snapshot(&self) {
        let _ = self.app.emit("combat:snapshot", self.pipeline.snapshot());
    }

    fn emit_status(&self) {
        let s = {
            let mut s = self.status.lock().unwrap();
            s.stats = self.pipeline.stats();
            s.dropped_segments = self.dropped.load(Ordering::Relaxed);
            if let Some(own) = self.pipeline.own_character() {
                s.own_character = Some(own.clone());
            }
            s.clone()
        };
        let _ = self.app.emit("app:status", s);
    }

    fn set_state(&self, state: CaptureState, msg: &str) {
        {
            let mut s = self.status.lock().unwrap();
            s.state = state;
            s.message = msg.to_owned();
        }
        self.emit_status();
    }
}

fn new_pipeline(filter: ServerFilter) -> Pipeline {
    Pipeline::new(OpcodeTable::default(), filter, EngineConfig::default())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
