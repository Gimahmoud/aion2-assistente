import { BellRing } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { cn } from "@/lib/utils";
import {
  fmtCountdown,
  fmtTime,
  fmtWeekdayTime,
  trackerApi,
  useNow,
  type EventConfig,
  type UpcomingEvent,
} from "@/lib/tracker";

function LeadInput({ value, onCommit }: { value: number; onCommit: (v: number) => void }) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const commit = () => {
    const n = Math.max(0, Math.min(1440, Math.round(Number(draft) || 0)));
    setDraft(String(n));
    if (n !== value) onCommit(n);
  };
  return (
    <Input
      className="h-8 w-20"
      type="number"
      min={0}
      max={1440}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
      aria-label="Minutos de antecedência"
    />
  );
}

export function EventsTab() {
  const [events, setEvents] = useState<EventConfig[]>([]);
  const [upcoming, setUpcoming] = useState<UpcomingEvent[]>([]);
  const [testError, setTestError] = useState<string | null>(null);
  const now = useNow(1000);
  const minute = Math.floor(now / 60_000);

  const reload = useCallback(() => {
    trackerApi
      .listEvents()
      .then(setEvents)
      .catch(() => {});
    trackerApi
      .upcomingEvents(24)
      .then(setUpcoming)
      .catch(() => {});
  }, []);
  // Recalcula a cada minuto para tirar da lista o que já passou.
  useEffect(reload, [reload, minute]);

  async function save(e: EventConfig, enabled: boolean, lead: number) {
    setEvents((list) => list.map((x) => (x.id === e.id ? { ...x, enabled, lead_min: lead } : x)));
    await trackerApi.setEventAlert(e.id, enabled, lead).catch(() => {});
    reload();
  }

  const visible = upcoming.filter((u) => (u.end_ms ?? u.start_ms) > now).slice(0, 30);

  return (
    <div className="flex flex-col gap-4">
      <Card className="gap-2">
        <CardHeader>
          <CardTitle>Próximas 24 horas</CardTitle>
          <CardDescription>Horários no relógio do seu computador.</CardDescription>
        </CardHeader>
        <CardContent>
          {visible.length === 0 ? (
            <p className="text-muted-foreground text-sm">Nenhum evento nas próximas 24 horas.</p>
          ) : (
            <ul className="divide-y">
              {visible.map((u) => {
                const ongoing = u.start_ms <= now;
                return (
                  <li key={`${u.event_id}-${u.start_ms}`} className="flex items-center gap-3 py-1.5 text-sm">
                    <span className="w-24 tabular-nums">{fmtWeekdayTime(u.start_ms)}</span>
                    <span className={cn("flex-1", !u.enabled && "text-muted-foreground")}>{u.nome}</span>
                    {ongoing ? (
                      <Badge>agora{u.end_ms ? ` até ${fmtTime(u.end_ms)}` : ""}</Badge>
                    ) : (
                      <span className="text-muted-foreground tabular-nums">em {fmtCountdown(u.start_ms - now)}</span>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </CardContent>
      </Card>

      <Card className="gap-2">
        <CardHeader>
          <CardTitle>Alertas</CardTitle>
          <CardDescription>
            O app avisa com uma notificação do Windows antes de cada evento ligado, enquanto estiver aberto.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Avisar</TableHead>
                <TableHead>Evento</TableHead>
                <TableHead>Minutos antes</TableHead>
                <TableHead>Próximo</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {events.map((e) => (
                <TableRow key={e.id}>
                  <TableCell>
                    <Switch checked={e.enabled} onCheckedChange={(v) => save(e, v, e.lead_min)} aria-label={e.nome} />
                  </TableCell>
                  <TableCell className="whitespace-normal">
                    <div className="font-medium">{e.nome}</div>
                    {e.descricao && <div className="text-muted-foreground text-xs">{e.descricao}</div>}
                  </TableCell>
                  <TableCell>
                    <LeadInput value={e.lead_min} onCommit={(v) => save(e, e.enabled, v)} />
                  </TableCell>
                  <TableCell className="tabular-nums">
                    {e.next_start_ms ? fmtWeekdayTime(e.next_start_ms) : "-"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                trackerApi
                  .testNotification()
                  .then(() => setTestError(null))
                  .catch((err) => setTestError(String(err)))
              }
            >
              <BellRing /> Testar notificação
            </Button>
            {testError && <span className="text-destructive text-sm">{testError}</span>}
          </div>
          <p className="text-muted-foreground text-xs">
            Os horários seguem o calendário fixo do servidor global. Se algum não bater com o jogo, avise para
            corrigirmos.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
