import { useEffect, useState } from "react";

import { MeterBars } from "@/components/MeterBars";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { className_, fmtCompact, fmtDuration, fmtNumber, fmtPercent } from "@/lib/format";
import type { EncounterSnapshot } from "@/lib/types";

/** Luta completa: barras por jogador e, ao clicar, o detalhe por skill. */
export function EncounterDetail({ encounter }: { encounter: EncounterSnapshot }) {
  const [selected, setSelected] = useState<number | null>(null);
  useEffect(() => setSelected(null), [encounter.id]);

  const actor = encounter.actors.find((a) => a.entity === selected) ?? encounter.actors.find((a) => a.is_self);
  const target = encounter.target;

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
      <Card className="gap-4">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Luta #{encounter.id}
            {encounter.live ? <Badge>ao vivo</Badge> : <Badge variant="secondary">encerrada</Badge>}
          </CardTitle>
          <CardDescription>
            {fmtDuration(encounter.duration_ms)} · dano total {fmtCompact(encounter.total_damage)}
            {target && ` · alvo ${target.mob_code ?? `#${target.entity}`}`}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {encounter.actors.length ? (
            <MeterBars actors={encounter.actors} selected={actor?.entity} onSelect={setSelected} />
          ) : (
            <p className="text-muted-foreground text-sm">Sem dano registrado.</p>
          )}
        </CardContent>
      </Card>

      {actor && (
        <Card className="gap-4">
          <CardHeader>
            <CardTitle>{actor.name}</CardTitle>
            <CardDescription>
              {className_(actor.class)} · {fmtNumber(actor.dps)} DPS · {fmtNumber(actor.hits)} golpes · maior{" "}
              {fmtNumber(actor.max_hit)} · perfeitos {fmtPercent(actor.hits ? actor.perfect_hits / actor.hits : 0)}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Skill</TableHead>
                  <TableHead className="text-right">Dano</TableHead>
                  <TableHead className="text-right">%</TableHead>
                  <TableHead className="text-right">Golpes</TableHead>
                  <TableHead className="text-right">Maior</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {actor.skills.map((s) => (
                  <TableRow key={s.skill}>
                    <TableCell className="font-mono text-xs">
                      {s.skill}
                      {s.dot && (
                        <Badge variant="outline" className="ml-2">
                          DoT
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{fmtNumber(s.damage)}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {fmtPercent(actor.damage ? s.damage / actor.damage : 0)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{s.hits}</TableCell>
                    <TableCell className="text-right tabular-nums">{fmtNumber(s.max_hit)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
