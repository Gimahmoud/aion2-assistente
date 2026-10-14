import type { ClassId } from "./types";

const compact = new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 });
const full = new Intl.NumberFormat("pt-BR");

export const fmtCompact = (n: number) => compact.format(n);
export const fmtNumber = (n: number) => full.format(Math.round(n));
export const fmtPercent = (f: number) => `${(f * 100).toFixed(1)}%`;

export function fmtDuration(ms: number) {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export const fmtDateTime = (ms: number) =>
  new Date(ms).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "medium" });

export const CLASS_NAMES: Record<ClassId, string> = {
  gladiator: "Gladiador",
  templar: "Templário",
  assassin: "Assassino",
  ranger: "Arqueiro",
  sorcerer: "Feiticeiro",
  spiritmaster: "Espiritualista",
  cleric: "Clérigo",
  chanter: "Cantor",
};

/** Cor da barra por classe (Tailwind). */
export const CLASS_COLORS: Record<ClassId, string> = {
  gladiator: "bg-orange-500",
  templar: "bg-sky-500",
  assassin: "bg-violet-500",
  ranger: "bg-lime-500",
  sorcerer: "bg-fuchsia-500",
  spiritmaster: "bg-teal-500",
  cleric: "bg-yellow-400",
  chanter: "bg-emerald-500",
};

export const classColor = (c: ClassId | null) => (c ? CLASS_COLORS[c] : "bg-zinc-500");
export const className_ = (c: ClassId | null) => (c ? CLASS_NAMES[c] : "Desconhecida");
