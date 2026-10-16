import { Radio } from "lucide-react";
import { useState } from "react";

import { EncounterDetail } from "@/components/EncounterDetail";
import { Button } from "@/components/ui/button";
import { useEvent, useRecentEncounters } from "@/lib/api";
import { fmtCompact, fmtDuration } from "@/lib/format";
import { targetLabel } from "@/lib/mobs";
import type { EncounterSnapshot } from "@/lib/types";

const RECENT = 8;

/**
 * Luta ao vivo e as últimas encerradas. Por padrão acompanha a luta atual e,
 * entre uma e outra, mantém a última na tela. Clicar numa luta recente fixa
 * ela até voltar para "Ao vivo", mesmo que outra comece.
 */
export function MeterTab() {
  const live = useEvent<EncounterSnapshot | null>("combat:snapshot", null);
  const recent = useRecentEncounters(RECENT);
  const [pinned, setPinned] = useState<number | null>(null);

  const pinnedEncounter = recent.find((r) => r.db_id === pinned)?.encounter ?? null;
  const following = pinnedEncounter === null;
  const shown = pinnedEncounter ?? live ?? recent[0]?.encounter ?? null;

  if (!shown) {
    return (
      <p className="text-muted-foreground p-6 text-sm">
        Nenhuma luta ainda. Ataque algo no jogo e o DPS aparece aqui. Atalhos: Ctrl+Shift+R inicia uma nova luta,
        Ctrl+Shift+O destrava o overlay para mover.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" variant={following ? "default" : "outline"} onClick={() => setPinned(null)}>
          <Radio /> {live ? "Ao vivo" : "Acompanhar"}
        </Button>
        {recent.map((r) => (
          <Button
            key={r.db_id}
            size="sm"
            variant={pinned === r.db_id ? "secondary" : "ghost"}
            onClick={() => setPinned(r.db_id)}
            title="Ver esta luta"
          >
            <span className="max-w-40 truncate">{targetLabel(r.encounter.target)}</span>
            <span className="text-muted-foreground tabular-nums">
              {fmtDuration(r.encounter.duration_ms)} · {fmtCompact(r.encounter.total_damage)}
            </span>
          </Button>
        ))}
      </div>
      <EncounterDetail encounter={shown} />
    </div>
  );
}
