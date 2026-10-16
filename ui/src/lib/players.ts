// Aba Jogadores: quem apareceu nas suas lutas e o perfil oficial de cada um.

import { invoke } from "@tauri-apps/api/core";

import { searchOfficial, type Hit } from "@/lib/official";
import { pick, text, unwrap, type Json, type Obj } from "@/lib/profile";
import type { CachedProfile, OfficialLink } from "@/lib/tracker";
import type { ClassId, EncounterSnapshot } from "@/lib/types";

export interface SeenPlayer {
  name: string;
  class: ClassId | null;
  isSelf: boolean;
  fights: number;
  lastSeenMs: number;
  /** DPS na luta mais recente em que apareceu. */
  lastDps: number;
}

/** Nome de verdade (o medidor usa `#123` quando ainda não sabe o nome). */
const realName = (n: string) => n.trim() !== "" && !n.startsWith("#") && n !== "Eu";

/** Junta os jogadores das lutas, da mais recente para a mais antiga. */
export function collectPlayers(encounters: EncounterSnapshot[]): SeenPlayer[] {
  const byName = new Map<string, SeenPlayer>();
  const sorted = [...encounters].sort((a, b) => b.started_at_ms - a.started_at_ms);
  const seenFight = new Set<number>();
  for (const enc of sorted) {
    if (seenFight.has(enc.id)) continue;
    seenFight.add(enc.id);
    for (const a of enc.actors) {
      if (!realName(a.name)) continue;
      const key = a.name.toLowerCase();
      const cur = byName.get(key);
      if (cur) {
        cur.fights += 1;
        cur.class ??= a.class;
        cur.isSelf ||= a.is_self;
      } else {
        byName.set(key, {
          name: a.name,
          class: a.class,
          isSelf: a.is_self,
          fights: 1,
          lastSeenMs: enc.started_at_ms,
          lastDps: a.dps,
        });
      }
    }
  }
  return [...byName.values()].sort((a, b) => b.lastSeenMs - a.lastSeenMs || b.lastDps - a.lastDps);
}

/** Resumo do perfil oficial (os mesmos campos da aba Perfil). */
export interface ProfileSummary {
  level?: string;
  className?: string;
  server?: string;
  legion?: string;
  combatPower?: string;
  itemLevel?: string;
}

export function summarize(cached: CachedProfile): ProfileSummary {
  let parsed: { info?: Json } = {};
  try {
    parsed = JSON.parse(cached.json);
  } catch {
    return {};
  }
  const info = unwrap(parsed.info);
  const profileKey = Object.keys(info).find((k) => /^(profile|character(info)?)$/i.test(k));
  const profile = (profileKey ? info[profileKey] : info) as Obj;
  // Poder de combate e nível de item podem vir no perfil ou na lista de atributos.
  const statKey = Object.keys(info).find((k) => /^stat/i.test(k));
  const stats = statKey ? info[statKey] : undefined;
  const statList = (Array.isArray(stats) ? stats : (pick(stats, ["statList", "list"]) as Json)) ?? [];
  const fromStats = (re: RegExp) => {
    if (!Array.isArray(statList)) return undefined;
    for (const s of statList) {
      const name = text(pick(s, ["name", "statName", "type"]));
      if (name && re.test(name)) return text(pick(s, ["value", "statValue", "amount"]));
    }
    return undefined;
  };
  return {
    level: text(pick(profile, ["characterLevel", "level"])),
    className: text(pick(profile, ["className", "jobName", "classNameText"])),
    server: text(pick(profile, ["serverName", "server"])),
    legion: text(pick(profile, ["legionName", "guildName"])),
    combatPower:
      text(pick(profile, ["combatPower", "power", "cp", "battlePower"])) ?? fromStats(/combat\s*power|poder de combate/i),
    itemLevel: text(pick(profile, ["itemLevel", "gearScore"])) ?? fromStats(/item\s*level|n[ií]vel de item/i),
  };
}

export type Lookup =
  | { state: "loading" }
  | { state: "found"; hit: Hit; profile: CachedProfile; summary: ProfileSummary }
  | { state: "choose"; hits: Hit[] }
  | { state: "missing" }
  | { state: "error"; message: string };

export const fetchProfile = (link: OfficialLink) => invoke<CachedProfile>("official_fetch", { link });

export async function openHit(hit: Hit): Promise<Lookup> {
  if (!hit.characterId || !hit.serverId || !hit.region) {
    return { state: "error", message: "A busca não trouxe o identificador do personagem." };
  }
  const profile = await fetchProfile({ region: hit.region, server_id: hit.serverId, character_id: hit.characterId });
  return { state: "found", hit, profile, summary: summarize(profile) };
}

/**
 * Procura o jogador pelo nome. Se houver um só com esse nome (ou um só no
 * mesmo servidor que o seu), abre o perfil direto; senão pede para escolher.
 */
export async function lookupPlayer(name: string, preferredServerId?: string): Promise<Lookup> {
  const hits = (await searchOfficial("", name)).filter((h) => h.name.toLowerCase() === name.toLowerCase());
  if (hits.length === 0) return { state: "missing" };
  const sameServer = preferredServerId ? hits.filter((h) => h.serverId === preferredServerId) : [];
  const pickOne = hits.length === 1 ? hits[0] : sameServer.length === 1 ? sameServer[0] : undefined;
  if (!pickOne) return { state: "choose", hits };
  return openHit(pickOne);
}
