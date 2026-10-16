import names from "@/lib/mob-names.pt-BR.json";
import type { EncounterSnapshot } from "@/lib/types";

const NAMES: Record<string, string> = names;

/** Nome do monstro ou boss pelo código que o jogo manda (ex.: 2300812). */
export function mobName(code: number | null | undefined): string | null {
  return code == null ? null : (NAMES[String(code)] ?? null);
}

/** Como mostrar o alvo principal de uma luta. */
export function targetLabel(target: EncounterSnapshot["target"]): string {
  if (!target) return "sem alvo";
  if (target.mob_code == null) return `Alvo #${target.entity}`;
  return mobName(target.mob_code) ?? `Monstro ${target.mob_code}`;
}
