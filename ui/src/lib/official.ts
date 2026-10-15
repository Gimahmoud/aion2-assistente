// Resultados da busca no site oficial e a ligação com um personagem do app.

import { CLASS_NAMES } from "@/lib/format";
import { isObj, pick, text, type Obj } from "@/lib/profile";
import { trackerApi, type Character } from "@/lib/tracker";

export interface Hit {
  raw: Obj;
  name: string;
  characterId?: string;
  serverId?: string;
  region?: string;
  className?: string;
  classId: string | null;
  level?: string;
  server?: string;
  race?: string;
}

/** A busca devolve o nome com marcação de destaque (`<b>Gim</b>ahmoud`). */
export const cleanName = (v: string) => v.replace(/<[^>]*>/g, "").trim();

function decode(v: string) {
  try {
    return decodeURIComponent(v);
  } catch {
    return v;
  }
}

export function toHit(raw: Obj): Hit {
  const className = text(pick(raw, ["className", "jobName", "classNameText"]));
  const key = className?.toLowerCase().replace(/\s+/g, "");
  const characterId = text(pick(raw, ["characterId", "charId", "id"]));
  return {
    raw,
    name: cleanName(text(pick(raw, ["name", "characterName"])) ?? "?"),
    characterId: characterId && decode(characterId),
    serverId: text(pick(raw, ["serverId", "server_id"])),
    region: text(pick(raw, ["region"])),
    className,
    classId: key && key in CLASS_NAMES ? key : null,
    level: text(pick(raw, ["level", "characterLevel"])),
    server: text(pick(raw, ["serverName", "server"])),
    race: text(pick(raw, ["raceName", "race"])),
  };
}

export async function searchOfficial(region: string, name: string): Promise<Hit[]> {
  const list = await trackerApi.officialSearch(region || null, name);
  return list.filter(isObj).map((r) => toHit(r as Obj));
}

/** Liga o personagem ao perfil oficial e já baixa o perfil. */
export async function linkHit(character: Character, hit: Hit) {
  if (!hit.characterId || !hit.serverId || !hit.region) {
    throw new Error('A busca não trouxe o identificador do personagem. Use "Copiar dados" e mande no chat.');
  }
  await trackerApi.officialLink(character.id, {
    region: hit.region,
    server_id: hit.serverId,
    character_id: hit.characterId,
  });
  await trackerApi.officialRefresh(character.id).catch(() => {});
}

/** Abre o resultado: usa o personagem com o mesmo nome ou cria um. */
export async function openHit(chars: Character[], hit: Hit): Promise<Character> {
  const same = chars.find((c) => c.name.toLowerCase() === hit.name.toLowerCase());
  const character = same ?? (await trackerApi.addCharacter(hit.name, hit.classId));
  await linkHit(character, hit);
  return character;
}
