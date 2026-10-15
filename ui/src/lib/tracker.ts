// Tracker de personagem e eventos: tipos e chamadas ao backend (src-tauri/src/tracker_cmds.rs).
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

export type Period = "diario" | "semanal";

export interface OfficialLink {
  region: string;
  server_id: string;
  character_id: string;
}

export interface Character {
  id: number;
  name: string;
  class: string | null;
  official: OfficialLink | null;
}

/** Cópia do perfil oficial: `json` é `{ info, equipment }` como veio da API. */
export interface CachedProfile {
  json: string;
  fetched_at_ms: number;
}

export interface TaskStatus {
  id: string;
  nome: string;
  periodo: Period;
  meta: number;
  escopo: "personagem" | "servidor";
  custom: boolean;
  count: number;
}

export interface Checklist {
  next_daily_reset_ms: number;
  next_weekly_reset_ms: number;
  tasks: TaskStatus[];
}

export interface PetStatus {
  id: string;
  nome: string;
  grupo: string;
  origem: string;
  custom: boolean;
  owned: boolean;
}

export interface EventConfig {
  id: string;
  nome: string;
  descricao: string | null;
  enabled: boolean;
  lead_min: number;
  next_start_ms: number | null;
}

export interface UpcomingEvent {
  event_id: string;
  nome: string;
  start_ms: number;
  end_ms: number | null;
  enabled: boolean;
}

export const trackerApi = {
  listCharacters: () => invoke<Character[]>("list_characters"),
  addCharacter: (name: string, cls: string | null) => invoke<Character>("add_character", { name, class: cls }),
  updateCharacter: (id: number, name: string, cls: string | null) =>
    invoke<void>("update_character", { id, name, class: cls }),
  removeCharacter: (id: number) => invoke<void>("remove_character", { id }),
  getChecklist: (characterId: number) => invoke<Checklist>("get_checklist", { characterId }),
  setTaskCount: (characterId: number, taskId: string, count: number) =>
    invoke<void>("set_task_count", { characterId, taskId, count }),
  addCustomTask: (name: string, period: Period, goal: number) =>
    invoke<string>("add_custom_task", { name, period, goal }),
  removeCustomTask: (id: string) => invoke<void>("remove_custom_task", { id }),
  getPets: (characterId: number) => invoke<PetStatus[]>("get_pets", { characterId }),
  setPetOwned: (characterId: number, petId: string, owned: boolean) =>
    invoke<void>("set_pet_owned", { characterId, petId, owned }),
  addCustomPet: (name: string, group: string, origin: string) =>
    invoke<string>("add_custom_pet", { name, group, origin }),
  removeCustomPet: (id: string) => invoke<void>("remove_custom_pet", { id }),
  listEvents: () => invoke<EventConfig[]>("list_events"),
  upcomingEvents: (hours = 24) => invoke<UpcomingEvent[]>("upcoming_events", { hours }),
  setEventAlert: (eventId: string, enabled: boolean, leadMin: number) =>
    invoke<void>("set_event_alert", { eventId, enabled, leadMin }),
  testNotification: () => invoke<void>("test_notification"),
  officialSearch: (region: string, name: string) =>
    invoke<Record<string, unknown>[]>("official_search", { region, name }),
  officialLink: (characterId: number, link: OfficialLink | null) =>
    invoke<void>("official_link", { characterId, link }),
  officialRefresh: (characterId: number) => invoke<CachedProfile>("official_refresh", { characterId }),
  officialCached: (characterId: number) => invoke<CachedProfile | null>("official_cached", { characterId }),
};

/** Hora atual, atualizada a cada `everyMs`. */
export function useNow(everyMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}

/** "2d 3h", "4h 05min", "12 min", "45 s". */
export function fmtCountdown(ms: number): string {
  if (ms <= 0) return "agora";
  const s = Math.floor(ms / 1000);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}min`;
  if (m > 0) return `${m} min`;
  return `${s % 60} s`;
}

/** Dia da semana e hora no relógio do PC, ex.: "qua 04:00". */
export const fmtWeekdayTime = (ms: number) =>
  new Date(ms).toLocaleString("pt-BR", { weekday: "short", hour: "2-digit", minute: "2-digit" }).replace(".", "");

export const fmtTime = (ms: number) => new Date(ms).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
