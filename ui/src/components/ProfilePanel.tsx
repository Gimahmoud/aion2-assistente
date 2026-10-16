import { Copy, ExternalLink, Link2Off, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { OfficialSearch } from "@/components/OfficialSearch";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { fmtDateTime, fmtNumber } from "@/lib/format";
import {
  findLists,
  isObj,
  itemDetails,
  itemIcon,
  itemLabel,
  LABEL_KEYS,
  pick,
  sectionName,
  text,
  unwrap,
  type Json,
  type Obj,
} from "@/lib/profile";
import { linkHit } from "@/lib/official";
import { trackerApi, type CachedProfile, type Character } from "@/lib/tracker";

function LinkForm({ character, onLinked }: { character: Character; onLinked: () => void }) {
  return (
    <Card className="gap-3">
      <CardHeader>
        <CardTitle>Perfil oficial</CardTitle>
        <CardDescription>
          Procurando {character.name} no site oficial do Aion 2 para mostrar nível, poder de combate, atributos,
          equipamentos, mascote e o resto do perfil aqui no app. Se aparecer mais de um, escolha o seu.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <OfficialSearch
          initialName={character.name}
          autoSearch
          autoPick={(hits) => {
            const same = hits.filter((h) => h.name.toLowerCase() === character.name.toLowerCase());
            return same.length === 1 ? same[0] : undefined;
          }}
          chooseLabel="É este"
          onChoose={async (hit) => {
            await linkHit(character, hit);
            onLinked();
          }}
        />
      </CardContent>
    </Card>
  );
}

function Companions({ equipment }: { equipment: Obj }) {
  const pw = pick(equipment, ["petwing", "petWing"]);
  if (!isObj(pw)) return null;
  const items = [
    ["Mascote", pick(pw, ["pet"])],
    ["Asas", pick(pw, ["wing"])],
  ].filter(([, v]) => isObj(v) && text(pick(v, LABEL_KEYS))) as [string, Obj][];
  if (items.length === 0) return null;
  return (
    <Card className="gap-2">
      <CardHeader>
        <CardTitle className="text-base">Mascote e asas equipados</CardTitle>
        <CardDescription>
          O site oficial só mostra o mascote equipado. Ele é marcado sozinho como obtido na aba Mascotes.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-1.5">
        {items.map(([label, it]) => (
          <div key={label} className="flex items-center gap-2 text-sm">
            <span className="text-muted-foreground w-16">{label}</span>
            <ItemList items={[it]} />
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function ItemList({ items }: { items: Obj[] }) {
  return (
    <ul className="grid gap-x-4 gap-y-1.5 sm:grid-cols-2">
      {items.map((it, i) => {
        const icon = itemIcon(it);
        return (
          <li key={i} className="flex items-center gap-2 text-sm">
            {icon ? (
              <img src={icon} alt="" className="bg-muted size-8 shrink-0 rounded" loading="lazy" />
            ) : (
              <span className="bg-muted size-8 shrink-0 rounded" />
            )}
            <span className="min-w-0 flex-1 truncate">{itemLabel(it)}</span>
            {itemDetails(it).map((d) => (
              <Badge key={d} variant="outline">
                {d}
              </Badge>
            ))}
          </li>
        );
      })}
    </ul>
  );
}

function Section({ title, value }: { title: string; value: Json }) {
  const lists = findLists(value);
  const scalars = isObj(value)
    ? Object.entries(value).filter(([, v]) => !isObj(v) && !Array.isArray(v) && v !== null && v !== "")
    : [];
  if (lists.length === 0 && scalars.length === 0) return null;
  return (
    <Card className="gap-2">
      <CardHeader>
        <CardTitle className="text-base">{title}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {scalars.length > 0 && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            {scalars.map(([k, v]) => (
              <div key={k} className="contents">
                <dt className="text-muted-foreground">{k}</dt>
                <dd>{String(v)}</dd>
              </div>
            ))}
          </dl>
        )}
        {lists.map((l) => (
          <div key={l.path} className="flex flex-col gap-1">
            {lists.length > 1 && <p className="text-muted-foreground text-xs">{sectionName(l.path)}</p>}
            <ItemList items={l.items} />
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function Stats({ value }: { value: Json }) {
  const list = (isObj(value) ? pick(value, ["statList", "list"]) : value) as Json;
  if (!Array.isArray(list)) return <Section title="Atributos" value={value} />;
  const rows = list.filter(isObj).map((s) => ({
    name: text(pick(s, ["name", "statName", "type"])) ?? "?",
    value: text(pick(s, ["value", "statValue", "amount"])) ?? "",
  }));
  return (
    <Card className="gap-2">
      <CardHeader>
        <CardTitle className="text-base">Atributos</CardTitle>
      </CardHeader>
      <CardContent>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-3">
          {rows.map((r, i) => (
            <div key={i} className="flex justify-between gap-2">
              <dt className="text-muted-foreground truncate">{r.name}</dt>
              <dd className="tabular-nums">{/^\d+$/.test(r.value) ? fmtNumber(Number(r.value)) : r.value}</dd>
            </div>
          ))}
        </dl>
      </CardContent>
    </Card>
  );
}

export function ProfileView({ cached }: { cached: CachedProfile }) {
  const parsed = useMemo(() => {
    try {
      return JSON.parse(cached.json) as { info: Json; equipment: Json };
    } catch {
      return { info: null, equipment: null };
    }
  }, [cached.json]);
  const info = unwrap(parsed.info);
  const equipment = unwrap(parsed.equipment);
  const profileKey = Object.keys(info).find((k) => /^(profile|character(info)?)$/i.test(k));
  const statKey = Object.keys(info).find((k) => /^stat/i.test(k));
  const profile = (profileKey ? info[profileKey] : info) as Obj;

  const name = text(pick(profile, ["characterName", "name"]));
  const image = text(pick(profile, ["profileImage", "characterImage", "imageUrl", "image"]));
  const facts = [
    ["Nível", text(pick(profile, ["characterLevel", "level"]))],
    ["Classe", text(pick(profile, ["className", "jobName", "classNameText"]))],
    ["Raça", text(pick(profile, ["raceName", "race"]))],
    ["Servidor", text(pick(profile, ["serverName", "server"]))],
    ["Legião", text(pick(profile, ["legionName", "guildName"]))],
    ["Título", text(pick(profile, ["titleName", "title"]))],
    ["Poder de combate", text(pick(profile, ["combatPower", "power", "cp", "battlePower"]))],
    ["Nível de item", text(pick(profile, ["itemLevel", "gearScore"]))],
  ].filter(([, v]) => v) as [string, string][];

  const others = Object.entries(info).filter(([k]) => k !== profileKey && k !== statKey);

  return (
    <div className="flex flex-col gap-4">
      <Card className="gap-3">
        <CardContent className="flex flex-wrap items-center gap-4">
          {image && /^https?:\/\//.test(image) && (
            <img src={image} alt="" className="bg-muted size-20 rounded-lg object-cover" />
          )}
          <div className="flex flex-col gap-1">
            <p className="text-lg font-semibold">{name ?? "Personagem"}</p>
            <div className="flex flex-wrap gap-x-4 gap-y-1 text-sm">
              {facts.map(([k, v]) => (
                <span key={k}>
                  <span className="text-muted-foreground">{k}: </span>
                  {/^\d{4,}$/.test(v) ? fmtNumber(Number(v)) : v}
                </span>
              ))}
            </div>
          </div>
        </CardContent>
      </Card>
      {statKey && <Stats value={info[statKey]} />}
      <Companions equipment={equipment} />
      {Object.keys(equipment).length > 0 && <Section title="Equipamentos" value={equipment} />}
      {others.map(([k, v]) => (
        <Section key={k} title={sectionName(k)} value={v} />
      ))}
    </div>
  );
}

export function ProfilePanel({ character, onChanged }: { character: Character; onChanged: () => void }) {
  const [cached, setCached] = useState<CachedProfile | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const load = useCallback(() => {
    trackerApi
      .officialCached(character.id)
      .then(setCached)
      .catch(() => setCached(null));
  }, [character.id]);
  useEffect(load, [load, character.official?.character_id]);

  async function refresh() {
    setBusy(true);
    setError(null);
    try {
      setCached(await trackerApi.officialRefresh(character.id));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!character.official) {
    return <LinkForm character={character} onLinked={onChanged} />;
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" onClick={refresh} disabled={busy}>
          <RefreshCw className={busy ? "animate-spin" : undefined} /> Atualizar do site oficial
        </Button>
        {cached && (
          <span className="text-muted-foreground text-xs">Atualizado em {fmtDateTime(cached.fetched_at_ms)}</span>
        )}
        <span className="mr-auto" />
        {cached && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              navigator.clipboard
                .writeText(JSON.stringify(JSON.parse(cached.json), null, 2))
                .then(() => setCopied(true))
                .catch(() => {})
            }
            title="Copia os dados como vieram do site, para ajudar a melhorar esta tela"
          >
            <Copy /> {copied ? "Copiado" : "Copiar dados"}
          </Button>
        )}
        <Button
          size="sm"
          variant="ghost"
          onClick={async () => {
            await trackerApi.officialLink(character.id, null);
            onChanged();
          }}
        >
          <Link2Off /> Trocar perfil
        </Button>
      </div>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {cached ? (
        <ProfileView cached={cached} />
      ) : (
        <p className="text-muted-foreground text-sm">
          Ainda não baixado. Clique em <strong>Atualizar do site oficial</strong>.
        </p>
      )}
      <p className="text-muted-foreground flex items-center gap-1 text-xs">
        <ExternalLink className="size-3" /> Dados do site oficial do Aion 2 (NCSoft). O app só lê o perfil público.
      </p>
    </div>
  );
}
