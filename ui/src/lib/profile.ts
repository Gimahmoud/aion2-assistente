// Leitura tolerante do perfil oficial. A API da NCSoft não é documentada,
// então procuramos os campos por vários nomes e mostramos o resto como veio.

export type Json = null | boolean | number | string | Json[] | { [k: string]: Json };
export type Obj = { [k: string]: Json };

export const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);

/** Primeiro valor presente entre as chaves candidatas (ignora maiúsculas). */
export function pick(o: unknown, keys: string[]): Json | undefined {
  if (!isObj(o)) return undefined;
  const lower = new Map(Object.keys(o).map((k) => [k.toLowerCase(), k]));
  for (const k of keys) {
    const real = lower.get(k.toLowerCase());
    if (real !== undefined && o[real] !== null && o[real] !== "") return o[real];
  }
  return undefined;
}

export const text = (v: Json | undefined) =>
  v === undefined || v === null || isObj(v) || Array.isArray(v) ? undefined : String(v);

/** Desembrulha `{ data: ... }` / `{ queryResult: { data } }`. */
export function unwrap(v: unknown): Obj {
  let cur: unknown = v;
  for (let i = 0; i < 3 && isObj(cur); i++) {
    const inner = pick(cur, ["data", "queryResult", "result"]);
    if (isObj(inner)) cur = inner;
    else break;
  }
  return isObj(cur) ? cur : {};
}

export const LABEL_KEYS = ["name", "itemName", "titleName", "petName", "skillName", "wingName", "title", "statName", "type"];
export const ICON_KEYS = ["icon", "iconUrl", "image", "imageUrl", "img", "itemIcon"];
const DETAIL_KEYS: [string, string][] = [
  ["enchantLevel", "+"],
  ["enchant", "+"],
  ["level", "nv. "],
  ["grade", ""],
  ["gradeName", ""],
  ["slotName", ""],
  ["categoryName", ""],
];

export function itemLabel(o: Obj): string {
  return text(pick(o, LABEL_KEYS)) ?? "?";
}

export function itemIcon(o: Obj): string | undefined {
  const v = text(pick(o, ICON_KEYS));
  return v && /^https?:\/\//.test(v) ? v : undefined;
}

export function itemDetails(o: Obj): string[] {
  const out: string[] = [];
  for (const [k, prefix] of DETAIL_KEYS) {
    const v = text(pick(o, [k]));
    if (v !== undefined && v !== "0") out.push(prefix + v);
  }
  return out;
}

/** Listas de objetos dentro de um valor (até 2 níveis), com o caminho. */
export function findLists(v: Json, path = "", depth = 0): { path: string; items: Obj[] }[] {
  if (Array.isArray(v)) {
    const items = v.filter(isObj);
    return items.length ? [{ path, items }] : [];
  }
  if (!isObj(v) || depth > 2) return [];
  return Object.entries(v).flatMap(([k, inner]) => findLists(inner, path ? `${path}.${k}` : k, depth + 1));
}

const SECTION_NAMES: [RegExp, string][] = [
  [/pet|familiar/i, "Mascotes"],
  [/wing/i, "Asas"],
  [/title/i, "Títulos"],
  [/stigma/i, "Estigmas"],
  [/arcana/i, "Arcana"],
  [/daevanion/i, "Daevanion"],
  [/skill/i, "Habilidades"],
  [/rank/i, "Rankings"],
  [/legion|guild/i, "Legião"],
  [/equip|item/i, "Equipamentos"],
  [/stat/i, "Atributos"],
  [/collect/i, "Coleções"],
];

export function sectionName(key: string): string {
  return SECTION_NAMES.find(([re]) => re.test(key))?.[1] ?? key;
}
