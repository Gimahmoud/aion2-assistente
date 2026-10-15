import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { searchOfficial, type Hit } from "@/lib/official";

// Códigos de região da API global. Só "nae" está confirmado; por isso a busca
// começa em todas as regiões ao mesmo tempo.
const REGIONS: [string, string][] = [
  ["", "Todas as regiões"],
  ["la", "América do Sul"],
  ["nae", "América do Norte (Leste)"],
  ["naw", "América do Norte (Oeste)"],
  ["eu", "Europa"],
  ["as", "Ásia"],
];

interface Props {
  initialName?: string;
  /** Busca assim que aparece, com `initialName`. */
  autoSearch?: boolean;
  /** Se devolver um resultado, ele é escolhido sem clique. */
  autoPick?: (hits: Hit[]) => Hit | undefined;
  chooseLabel?: string;
  onChoose: (hit: Hit) => Promise<void>;
}

export function OfficialSearch({ initialName = "", autoSearch, autoPick, chooseLabel = "Abrir", onChoose }: Props) {
  const [region, setRegion] = useState("");
  const [name, setName] = useState(initialName);
  const [hits, setHits] = useState<Hit[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function choose(hit: Hit) {
    setBusy(true);
    setError(null);
    try {
      await onChoose(hit);
      setHits(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function run(q: string, r: string, allowAuto: boolean) {
    if (!q.trim()) return;
    setBusy(true);
    setError(null);
    let found: Hit[] = [];
    try {
      found = await searchOfficial(r, q);
      setHits(found);
    } catch (err) {
      setError(String(err));
      setHits(null);
    } finally {
      setBusy(false);
    }
    const pick = allowAuto ? autoPick?.(found) : undefined;
    if (pick) await choose(pick);
  }

  const started = useRef(false);
  useEffect(() => {
    if (autoSearch && !started.current) {
      started.current = true;
      void run(initialName, "", true);
    }
  }, []);

  return (
    <div className="flex flex-col gap-3">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(name, region, false);
        }}
        className="flex flex-wrap items-center gap-2"
      >
        <Input
          className="w-56"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nome do personagem"
          aria-label="Nome do personagem"
        />
        <NativeSelect value={region} onChange={(e) => setRegion(e.target.value)} aria-label="Região">
          {REGIONS.map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </NativeSelect>
        <Button type="submit" size="sm" disabled={busy || !name.trim()}>
          <Search /> {busy ? "Buscando…" : "Buscar"}
        </Button>
      </form>
      {error && <p className="text-destructive text-sm">{error}</p>}
      {hits && hits.length === 0 && (
        <p className="text-muted-foreground text-sm">Ninguém com esse nome no site oficial. Confira a grafia.</p>
      )}
      {hits && hits.length > 0 && (
        <ul className="divide-y">
          {hits.map((h, i) => (
            <li key={i} className="flex items-center gap-3 py-1.5 text-sm">
              <span className="font-medium">{h.name}</span>
              <span className="text-muted-foreground flex-1">
                {[h.server, h.race, h.className, h.level && `nv. ${h.level}`].filter(Boolean).join(" · ")}
              </span>
              <Button size="sm" variant="outline" disabled={busy} onClick={() => choose(h)}>
                {chooseLabel}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
