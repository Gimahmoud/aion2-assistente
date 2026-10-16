import { ExternalLink, RefreshCw, Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { ProfileView } from "@/components/ProfilePanel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { api, useEvent } from "@/lib/api";
import { className_, classColor, fmtDateTime, fmtNumber } from "@/lib/format";
import { collectPlayers, lookupPlayer, openHit, type Lookup, type SeenPlayer } from "@/lib/players";
import { trackerApi } from "@/lib/tracker";
import type { EncounterSnapshot } from "@/lib/types";

/** Quantas lutas do histórico entram na lista. */
const RECENT_FIGHTS = 30;

const num = (v?: string) => (v && /^\d{4,}$/.test(v) ? fmtNumber(Number(v)) : v);

export function PlayersTab({ live }: { live: EncounterSnapshot | null }) {
  const [saved, setSaved] = useState<EncounterSnapshot[]>([]);
  const [lookups, setLookups] = useState<Record<string, Lookup>>({});
  const [open, setOpen] = useState<string | null>(null);
  const [batch, setBatch] = useState(false);
  const [myServer, setMyServer] = useState<string | undefined>();
  const lastClosed = useEvent<unknown>("combat:closed", null);

  useEffect(() => {
    let cancelled = false;
    api
      .listEncounters(RECENT_FIGHTS)
      .then((rows) => Promise.all(rows.map((r) => api.getEncounter(r.id))))
      .then((list) => !cancelled && setSaved(list.filter((e): e is EncounterSnapshot => !!e)))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [lastClosed]);

  // Seu servidor no site oficial: ajuda a escolher entre jogadores com o mesmo nome.
  useEffect(() => {
    trackerApi
      .listCharacters()
      .then((chars) => setMyServer(chars.find((c) => c.official)?.official?.server_id))
      .catch(() => {});
  }, []);

  const players = useMemo(() => collectPlayers(live ? [live, ...saved] : saved), [live, saved]);

  const set = (name: string, l: Lookup) => setLookups((cur) => ({ ...cur, [name.toLowerCase()]: l }));

  const lookup = useCallback(
    async (name: string) => {
      set(name, { state: "loading" });
      try {
        set(name, await lookupPlayer(name, myServer));
      } catch (err) {
        set(name, { state: "error", message: String(err) });
      }
    },
    [myServer],
  );

  async function lookupAll() {
    setBatch(true);
    for (const p of players) {
      const cur = lookups[p.name.toLowerCase()];
      if (cur && cur.state !== "error") continue;
      await lookup(p.name);
    }
    setBatch(false);
  }

  const opened = open ? lookups[open.toLowerCase()] : undefined;

  return (
    <div className="flex flex-col gap-4">
      <Card className="gap-2">
        <CardHeader className="flex flex-wrap items-start gap-2">
          <div className="mr-auto flex flex-col gap-1.5">
            <CardTitle>Jogadores das suas lutas</CardTitle>
            <CardDescription>
              Quem lutou junto com você nas últimas {RECENT_FIGHTS} lutas. Consulte para ver classe, poder de combate e
              nível de item no site oficial.
            </CardDescription>
          </div>
          {players.length > 0 && (
            <Button size="sm" onClick={lookupAll} disabled={batch}>
              <RefreshCw className={batch ? "animate-spin" : undefined} /> Consultar todos
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {players.length === 0 ? (
            <p className="text-muted-foreground text-sm">
              Ninguém ainda. Os jogadores aparecem aqui depois de lutar com o medidor ligado.
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Jogador</TableHead>
                  <TableHead>Classe</TableHead>
                  <TableHead className="text-right">Nível</TableHead>
                  <TableHead className="text-right">Poder de combate</TableHead>
                  <TableHead className="text-right">Nível de item</TableHead>
                  <TableHead className="text-right">Último DPS</TableHead>
                  <TableHead>Visto em</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {players.map((p) => (
                  <PlayerRow
                    key={p.name}
                    player={p}
                    lookup={lookups[p.name.toLowerCase()]}
                    active={open?.toLowerCase() === p.name.toLowerCase()}
                    onLookup={() => lookup(p.name)}
                    onOpen={() => setOpen(p.name)}
                  />
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {open && opened && (
        <Card className="gap-3">
          <CardHeader className="flex items-center gap-2">
            <CardTitle className="mr-auto">{open}</CardTitle>
            <Button size="icon" variant="ghost" onClick={() => setOpen(null)} title="Fechar">
              <X />
            </Button>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {opened.state === "choose" && (
              <>
                <p className="text-sm">Há mais de um personagem com esse nome. Qual deles?</p>
                <ul className="flex flex-col gap-1">
                  {opened.hits.map((h, i) => (
                    <li key={i} className="flex items-center gap-2 text-sm">
                      <span className="flex-1">
                        {h.name}
                        <span className="text-muted-foreground">
                          {" "}
                          · {[h.className, h.level && `nv. ${h.level}`, h.server, h.race].filter(Boolean).join(" · ")}
                        </span>
                      </span>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={async () => {
                          set(open, { state: "loading" });
                          set(open, await openHit(h).catch((err) => ({ state: "error", message: String(err) }) as Lookup));
                        }}
                      >
                        É este
                      </Button>
                    </li>
                  ))}
                </ul>
              </>
            )}
            {opened.state === "found" && <ProfileView cached={opened.profile} />}
            {opened.state === "loading" && <p className="text-muted-foreground text-sm">Consultando o site oficial…</p>}
            {opened.state === "missing" && (
              <p className="text-muted-foreground text-sm">Não achei esse nome no site oficial.</p>
            )}
            {opened.state === "error" && <p className="text-destructive text-sm">{opened.message}</p>}
            <p className="text-muted-foreground flex items-center gap-1 text-xs">
              <ExternalLink className="size-3" /> Dados públicos do site oficial do Aion 2 (NCSoft). Nada fica salvo.
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function PlayerRow({
  player: p,
  lookup,
  active,
  onLookup,
  onOpen,
}: {
  player: SeenPlayer;
  lookup?: Lookup;
  active: boolean;
  onLookup: () => void;
  onOpen: () => void;
}) {
  const s = lookup?.state === "found" ? lookup.summary : undefined;
  const status =
    lookup?.state === "loading"
      ? "Consultando…"
      : lookup?.state === "missing"
        ? "Não encontrado"
        : lookup?.state === "choose"
          ? `${lookup.hits.length} com esse nome`
          : lookup?.state === "error"
            ? "Erro"
            : undefined;
  return (
    <TableRow data-state={active ? "selected" : undefined}>
      <TableCell className="font-medium">
        <span className="flex items-center gap-2">
          <span className={`size-2 shrink-0 rounded-full ${classColor(p.class)}`} />
          {p.name}
          {p.isSelf && <Badge variant="secondary">você</Badge>}
        </span>
      </TableCell>
      <TableCell>{s?.className ?? className_(p.class)}</TableCell>
      <TableCell className="text-right tabular-nums">{s?.level ?? "-"}</TableCell>
      <TableCell className="text-right tabular-nums">{num(s?.combatPower) ?? status ?? "-"}</TableCell>
      <TableCell className="text-right tabular-nums">{num(s?.itemLevel) ?? "-"}</TableCell>
      <TableCell className="text-right tabular-nums">{fmtNumber(p.lastDps)}</TableCell>
      <TableCell className="text-muted-foreground text-xs">
        {fmtDateTime(p.lastSeenMs)}
        {p.fights > 1 && ` · ${p.fights} lutas`}
      </TableCell>
      <TableCell className="text-right">
        <Button
          size="sm"
          variant={active ? "secondary" : "outline"}
          onClick={() => {
            if (!lookup || lookup.state === "error") onLookup();
            onOpen();
          }}
        >
          <Search /> Ver perfil
        </Button>
      </TableCell>
    </TableRow>
  );
}
