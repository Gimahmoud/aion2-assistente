import { getCurrentWindow } from "@tauri-apps/api/window";
import { Move } from "lucide-react";

import { MeterBars } from "@/components/MeterBars";
import { useEvent } from "@/lib/api";
import { fmtCompact, fmtDuration } from "@/lib/format";
import type { EncounterSnapshot } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Janela transparente sobre o jogo. Ctrl+Shift+O alterna o modo de edição. */
export default function Overlay() {
  const live = useEvent<EncounterSnapshot | null>("combat:snapshot", null);
  const closed = useEvent<{ encounter: EncounterSnapshot } | null>("combat:closed", null);
  const clickThrough = useEvent<boolean>("overlay:click_through", true);
  const shown = live ?? closed?.encounter ?? null;
  const editing = !clickThrough;

  return (
    <div
      // No modo de edição, arrastar em qualquer ponto move a janela.
      onMouseDown={(e) => editing && e.button === 0 && getCurrentWindow().startDragging()}
      className={cn(
        "flex h-screen flex-col gap-1 rounded-md bg-black/55 p-1.5 text-white",
        editing && "ring-primary cursor-move ring-2 select-none",
      )}
    >
      <div className="flex items-center gap-2 px-1 text-xs">
        {editing && <Move className="size-3" />}
        <span className="font-semibold">DPS</span>
        {shown && (
          <>
            <span className="tabular-nums opacity-80">{fmtDuration(shown.duration_ms)}</span>
            <span className="ml-auto tabular-nums opacity-80">{fmtCompact(shown.total_damage)}</span>
            {!shown.live && <span className="opacity-60">fim</span>}
          </>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {shown ? (
          <MeterBars actors={shown.actors} compact />
        ) : (
          <p className="px-1 text-xs opacity-70">Aguardando combate…</p>
        )}
      </div>
      {editing && <p className="px-1 text-[10px] opacity-70">Arraste para mover e puxe a borda para redimensionar. Trave no botão do app ou com Ctrl+Shift+O.</p>}
    </div>
  );
}
