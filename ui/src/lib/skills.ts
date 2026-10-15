import names from "@/lib/skill-names.pt-BR.json";
import type { SkillStats } from "@/lib/types";

const NAMES: Record<string, string> = names;

/** Família da skill: os 4 primeiros dígitos do código (ex.: 14340130 → 1434). */
export function skillFamily(code: number): number {
  return code >= 10_000_000 ? Math.floor(code / 10_000) : code;
}

export function skillName(family: number): string {
  return NAMES[String(family)] ?? `Skill ${family}`;
}

export interface SkillRow extends SkillStats {
  family: number;
  name: string;
}

/** Junta os níveis/variações de cada skill numa linha só, como o Ctrl+X do jogo. */
export function groupSkills(skills: SkillStats[]): SkillRow[] {
  const rows = new Map<string, SkillRow>();
  for (const s of skills) {
    const family = skillFamily(s.skill);
    const key = `${family}:${s.dot}`;
    const row = rows.get(key);
    if (row) {
      row.damage += s.damage;
      row.hits += s.hits;
      row.max_hit = Math.max(row.max_hit, s.max_hit);
    } else {
      rows.set(key, { ...s, family, name: skillName(family) });
    }
  }
  return [...rows.values()].sort((a, b) => b.damage - a.damage);
}
