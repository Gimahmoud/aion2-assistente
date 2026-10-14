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

/** Status atual: busca uma vez e depois segue o evento `app:status`. */
export function useStatus(): Status | null {
  const [initial, setInitial] = useState<Status | null>(null);
  const live = useEvent<Status | null>("app:status", null);
  useEffect(() => {
    api.getStatus().then(setInitial).catch(() => {});
  }, []);
  return live ?? initial;
}
