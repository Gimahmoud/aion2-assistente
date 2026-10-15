// Espelha as structs serializadas pelo backend Rust (crates/combat, storage, src-tauri).

export type ClassId =
  | "gladiator"
  | "templar"
  | "assassin"
  | "ranger"
  | "sorcerer"
  | "spiritmaster"
  | "cleric"
  | "chanter";

export interface SkillStats {
  skill: number;
  damage: number;
  hits: number;
  max_hit: number;
  dot: boolean;
}

export interface ActorStats {
  entity: number;
  name: string;
  class: ClassId | null;
  is_self: boolean;
  damage: number;
  dps: number;
  share: number;
  hits: number;
  max_hit: number;
  perfect_hits: number;
  back_hits: number;
  skills: SkillStats[];
}

export interface EncounterSnapshot {
  id: number;
  started_at_ms: number;
  duration_ms: number;
  total_damage: number;
  target: { entity: number; mob_code: number | null; hp: number | null } | null;
  actors: ActorStats[];
  live: boolean;
}

export interface EncounterRow {
  id: number;
  started_at_ms: number;
  duration_ms: number;
  total_damage: number;
  target_mob_code: number | null;
  self_dps: number | null;
}

export type CaptureState = "starting" | "npcap_missing" | "waiting_for_game" | "capturing" | "replaying" | "error";

export interface PipelineStats {
  segments: number;
  bytes: number;
  messages: number;
  compressed_batches: number;
  resyncs: number;
  tcp_gaps: number;
  decompress_errors: number;
  events: Record<string, number>;
  unknown_opcodes: [string, number][];
  opcode_table: string;
}

export interface Status {
  state: CaptureState;
  message: string;
  servers: string[];
  device: string | null;
  recording: string | null;
  npcap_download: string;
  stats: PipelineStats;
  dropped_segments: number;
  own_character: { name: string; server: number | null; class: ClassId | null } | null;
}
