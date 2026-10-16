import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import type { EncounterRow, EncounterSnapshot, Status } from "./types";

export const api = {
  getStatus: () => invoke<Status>("get_status"),
  resetEncounter: () => invoke<void>("reset_encounter"),
  listEncounters: (limit = 100) => invoke<EncounterRow[]>("list_encounters", { limit }),
  getEncounter: (id: number) => invoke<EncounterSnapshot | null>("get_encounter", { id }),
  replayFile: (path: string) => invoke<void>("replay_file", { path }),
  recordingsDir: () => invoke<string>("recordings_dir"),
  setRecording: (enabled: boolean) => invoke<string | null>("set_recording", { enabled }),
  setOverlayClickThrough: (enabled: boolean) => invoke<void>("set_overlay_click_through", { enabled }),
  toggleOverlay: () => invoke<boolean>("toggle_overlay"),
};

/** Valor mais recente de um evento emitido pelo backend. */
export function useEvent<T>(name: string, initial: T): T {
  const [value, setValue] = useState<T>(initial);
  useEffect(() => {
    let off: UnlistenFn | undefined;
    let cancelled = false;
    listen<T>(name, (e) => setValue(e.payload)).then((fn) => {
      if (cancelled) fn();
      else off = fn;
    });
    return () => {
      cancelled = true;
      off?.();
    };
  }, [name]);
  return value;
}

export interface ClosedEncounter {
  db_id: number;
  encounter: EncounterSnapshot;
}

/**
 * Últimas lutas encerradas, a mais nova primeiro. Começa com as salvas no
 * banco e recebe cada `combat:closed` (nenhuma se perde, mesmo seguidas).
 */
export function useRecentEncounters(limit: number): ClosedEncounter[] {
  const [list, setList] = useState<ClosedEncounter[]>([]);
  useEffect(() => {
    let off: UnlistenFn | undefined;
    let cancelled = false;
    const add = (items: ClosedEncounter[]) =>
      setList((prev) => {
        const seen = new Set(items.map((i) => i.db_id));
        return [...items, ...prev.filter((p) => !seen.has(p.db_id))]
          .sort((a, b) => b.db_id - a.db_id)
          .slice(0, limit);
      });
    listen<ClosedEncounter>("combat:closed", (e) => add([e.payload])).then((fn) => {
      if (cancelled) fn();
      else off = fn;
    });
    api
      .listEncounters(limit)
      .then((rows) =>
        Promise.all(rows.map((r) => api.getEncounter(r.id).then((e) => (e ? { db_id: r.id, encounter: e } : null)))),
      )
      .then((items) => {
        if (!cancelled) add(items.filter((i): i is ClosedEncounter => i !== null));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      off?.();
    };
  }, [limit]);
  return list;
}

/** Status atual: busca uma vez e depois segue o evento `app:status`. */
export function useStatus(): Status | null {
  const [initial, setInitial] = useState<Status | null>(null);
  const live = useEvent<Status | null>("app:status", null);
  useEffect(() => {
    api.getStatus().then(setInitial).catch(() => {});
  }, []);
  return live ?? initial;
}
