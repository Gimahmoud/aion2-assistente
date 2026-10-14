import { useCallback, useEffect, useState } from "react";

import { EncounterDetail } from "@/components/EncounterDetail";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { api, useEvent } from "@/lib/api";
import { fmtCompact, fmtDateTime, fmtDuration, fmtNumber } from "@/lib/format";
import type { EncounterRow, EncounterSnapshot } from "@/lib/types";

export function HistoryTab() {
  const [rows, setRows] = useState<EncounterRow[]>([]);
  const [open, setOpen] = useState<EncounterSnapshot | null>(null);
  const lastClosed = useEvent<unknown>("combat:closed", null);

  const reload = useCallback(() => {
    api.listEncounters(200).then(setRows).catch(() => {});
  }, []);
  useEffect(reload, [reload, lastClosed]);

  return (
    <div className="flex flex-col gap-4">
      {open && <EncounterDetail encounter={open} />}
      <Card className="gap-2">
        <CardHeader>
          <CardTitle>Lutas anteriores</CardTitle>
        </CardHeader>
        <CardContent>
          {rows.length === 0 ? (
            <p className="text-muted-foreground text-sm">Nenhuma luta salva ainda.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Quando</TableHead>
                  <TableHead>Duração</TableHead>
                  <TableHead>Alvo</TableHead>
                  <TableHead className="text-right">Dano total</TableHead>
                  <TableHead className="text-right">Meu DPS</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r) => (
                  <TableRow
                    key={r.id}
                    className="cursor-pointer"
                    onClick={() => api.getEncounter(r.id).then(setOpen)}
                  >
                    <TableCell>{fmtDateTime(r.started_at_ms)}</TableCell>
                    <TableCell className="tabular-nums">{fmtDuration(r.duration_ms)}</TableCell>
                    <TableCell className="font-mono text-xs">{r.target_mob_code ?? "-"}</TableCell>
                    <TableCell className="text-right tabular-nums">{fmtCompact(r.total_damage)}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {r.self_dps != null ? fmtNumber(r.self_dps) : "-"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
