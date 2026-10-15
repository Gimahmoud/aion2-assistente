import { Plus, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { trackerApi, type PetStatus } from "@/lib/tracker";

type Filter = "missing" | "owned" | "all";

function AddPetForm({ groups, onAdded }: { groups: string[]; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [group, setGroup] = useState("");
  const [origin, setOrigin] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    await trackerApi.addCustomPet(name, group, origin);
    setName("");
    setOrigin("");
    onAdded();
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
      <Input className="w-48" placeholder="Nome do mascote" value={name} onChange={(e) => setName(e.target.value)} />
      <Input
        className="w-36"
        placeholder="Grupo"
        list="grupos-mascote"
        value={group}
        onChange={(e) => setGroup(e.target.value)}
      />
      <datalist id="grupos-mascote">
        {groups.map((g) => (
          <option key={g} value={g} />
        ))}
      </datalist>
      <Input
        className="w-64"
        placeholder="Onde consegue (opcional)"
        value={origin}
        onChange={(e) => setOrigin(e.target.value)}
      />
      <Button type="submit" size="sm" variant="outline" disabled={!name.trim()}>
        <Plus /> Adicionar mascote
      </Button>
    </form>
  );
}

export function PetsPanel({ characterId }: { characterId: number }) {
  const [pets, setPets] = useState<PetStatus[]>([]);
  const [filter, setFilter] = useState<Filter>("missing");
  const [group, setGroup] = useState("");
  const [query, setQuery] = useState("");

  const reload = useCallback(() => {
    trackerApi
      .getPets(characterId)
      .then(setPets)
      .catch(() => {});
  }, [characterId]);
  useEffect(reload, [reload]);

  const groups = useMemo(() => [...new Set(pets.map((p) => p.grupo))], [pets]);
  const owned = pets.filter((p) => p.owned).length;
  const q = query.trim().toLowerCase();
  const shown = pets.filter(
    (p) =>
      (filter === "all" || (filter === "owned") === p.owned) &&
      (!group || p.grupo === group) &&
      (!q || p.nome.toLowerCase().includes(q) || p.origem.toLowerCase().includes(q)),
  );

  async function toggle(p: PetStatus, value: boolean) {
    setPets((list) => list.map((x) => (x.id === p.id ? { ...x, owned: value } : x)));
    await trackerApi.setPetOwned(characterId, p.id, value).catch(reload);
  }

  return (
    <div className="flex flex-col gap-4">
      <Card className="gap-3">
        <CardHeader>
          <CardTitle>
            Mascotes{" "}
            <span className="text-muted-foreground text-sm font-normal">
              {owned} de {pets.length} na lista
            </span>
          </CardTitle>
          <CardDescription>
            Marque os que você já tem. O mascote equipado no perfil oficial é marcado sozinho, porque o site não mostra
            a coleção inteira. A lista que vem com o app ainda é parcial; adicione os que faltarem no fim da página.
          </CardDescription>
          <div className="bg-muted mt-1 h-2 overflow-hidden rounded-full">
            <div
              className="bg-primary h-full transition-all"
              style={{ width: `${pets.length ? (owned / pets.length) * 100 : 0}%` }}
            />
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex rounded-md border p-0.5">
              {(
                [
                  ["missing", "Faltando"],
                  ["owned", "Tenho"],
                  ["all", "Todos"],
                ] as [Filter, string][]
              ).map(([f, label]) => (
                <Button key={f} size="sm" variant={filter === f ? "secondary" : "ghost"} onClick={() => setFilter(f)}>
                  {label}
                </Button>
              ))}
            </div>
            <NativeSelect value={group} onChange={(e) => setGroup(e.target.value)} aria-label="Grupo">
              <option value="">Todos os grupos</option>
              {groups.map((g) => (
                <option key={g} value={g}>
                  {g}
                </option>
              ))}
            </NativeSelect>
            <div className="relative">
              <Search className="text-muted-foreground absolute top-1/2 left-2.5 size-4 -translate-y-1/2" />
              <Input
                className="w-56 pl-8"
                placeholder="Buscar nome ou origem"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
          </div>
          {shown.length === 0 ? (
            <p className="text-muted-foreground py-4 text-sm">
              {filter === "missing" && pets.length > 0 ? "Nenhum mascote faltando nesta lista." : "Nada encontrado."}
            </p>
          ) : (
            <ul className="divide-y">
              {shown.map((p) => (
                <li key={p.id} className="flex items-center gap-3 py-1.5">
                  <Checkbox
                    id={`pet-${p.id}`}
                    checked={p.owned}
                    onCheckedChange={(v) => toggle(p, v === true)}
                  />
                  <label htmlFor={`pet-${p.id}`} className="flex flex-1 flex-col text-sm">
                    <span className="font-medium">{p.nome}</span>
                    {p.origem && <span className="text-muted-foreground text-xs">{p.origem}</span>}
                  </label>
                  <Badge variant="outline">{p.grupo}</Badge>
                  {p.custom && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={() => trackerApi.removeCustomPet(p.id).then(reload)}
                      aria-label="Remover mascote"
                    >
                      <Trash2 />
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
      <AddPetForm groups={groups} onAdded={reload} />
    </div>
  );
}
