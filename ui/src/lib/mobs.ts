import bosses from "@/lib/bosses.pt-BR.json";
import type { EncounterSnapshot } from "@/lib/types";

/** Chefes (campo, mundo e instância) pelo código que o jogo manda ao surgir. */
const BOSSES: Record<string, string> = bosses;

export function mobName(code: number | null | undefined): string | null {
  return code == null ? null : (BOSSES[String(code)] ?? null);
}

export function isBoss(code: number | null | undefined): boolean {
  return mobName(code) !== null;
}

/** Como mostrar o alvo principal de uma luta. */
export function targetLabel(target: EncounterSnapshot["target"]): string {
  if (!target) return "sem alvo";
  if (target.mob_code == null) return `Alvo #${target.entity}`;
  return mobName(target.mob_code) ?? `Monstro ${target.mob_code}`;
}
