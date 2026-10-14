import type { ActorStats } from "@/lib/types";
import { classColor, fmtCompact, fmtPercent } from "@/lib/format";
import { cn } from "@/lib/utils";

interface Props {
  actors: ActorStats[];
  compact?: boolean;
  selected?: number | null;
  onSelect?: (entity: number) => void;
}

/** Barras de DPS por jogador, proporcionais ao maior dano da luta. */
export function MeterBars({ actors, compact, selected, onSelect }: Props) {
  const top = actors[0]?.damage || 1;
  return (
    <ol className={cn("flex flex-col", compact ? "gap-0.5" : "gap-1")}>
      {actors.map((a, i) => (
        <li key={a.entity}>
          <button
            type="button"
            disabled={!onSelect}
            onClick={() => onSelect?.(a.entity)}
            className={cn(
              "relative block w-full overflow-hidden rounded-sm text-left",
              compact ? "h-5 text-xs" : "h-8 text-sm",
              onSelect && "cursor-pointer hover:ring-1 hover:ring-ring",
              selected === a.entity && "ring-2 ring-primary",
            )}
          >
            <span
              className={cn("absolute inset-y-0 left-0 opacity-70", classColor(a.class))}
              style={{ width: `${(a.damage / top) * 100}%` }}
            />
            <span className="relative flex h-full items-center gap-2 px-2 font-medium text-white [text-shadow:0_1px_2px_rgb(0_0_0/80%)]">
              <span className="w-4 tabular-nums opacity-70">{i + 1}</span>
              <span className={cn("flex-1 truncate", a.is_self && "text-primary")}>{a.name}</span>
              <span className="tabular-nums">{fmtCompact(a.dps)}/s</span>
              <span className="w-12 text-right tabular-nums opacity-80">{fmtPercent(a.share)}</span>
            </span>
          </button>
        </li>
      ))}
    </ol>
  );
}
