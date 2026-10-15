import { Plus, Search, Trash2, UserPlus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { ChecklistPanel } from "@/components/ChecklistPanel";
import { OfficialSearch } from "@/components/OfficialSearch";
import { PetsPanel } from "@/components/PetsPanel";
import { ProfilePanel } from "@/components/ProfilePanel";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useStatus } from "@/lib/api";
import { CLASS_NAMES } from "@/lib/format";
import { openHit } from "@/lib/official";
import { trackerApi, type Character } from "@/lib/tracker";

const SELECTED_KEY = "personagem-selecionado";

function readSelected(): number | null {
  try {
    const v = Number(localStorage.getItem(SELECTED_KEY));
    return Number.isFinite(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

function writeSelected(id: number) {
  try {
    localStorage.setItem(SELECTED_KEY, String(id));
  } catch {
    /* sem armazenamento: volta para o primeiro personagem */
  }
}

function ClassSelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <NativeSelect value={value} onChange={(e) => onChange(e.target.value)} aria-label="Classe">
      <option value="">Classe</option>
      {Object.entries(CLASS_NAMES).map(([id, name]) => (
        <option key={id} value={id}>
          {name}
        </option>
      ))}
    </NativeSelect>
  );
}

function NewCharacterForm({ onCreated, onCancel }: { onCreated: (c: Character) => void; onCancel?: () => void }) {
  const [name, setName] = useState("");
  const [cls, setCls] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    try {
      onCreated(await trackerApi.addCharacter(name, cls || null));
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
      <Input
        className="w-56"
        placeholder="Nome do personagem"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <ClassSelect value={cls} onChange={setCls} />
      <Button type="submit" size="sm" disabled={!name.trim()}>
        <Plus /> Adicionar
      </Button>
      {onCancel && (
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancelar
        </Button>
      )}
      {error && <p className="text-destructive w-full text-sm">{error}</p>}
    </form>
  );
}

export function CharactersTab() {
  const [chars, setChars] = useState<Character[] | null>(null);
  const [selected, setSelected] = useState<number | null>(readSelected);
  const [adding, setAdding] = useState(false);
  const [searching, setSearching] = useState(false);
  const [tab, setTab] = useState("profile");
  const [confirmRemove, setConfirmRemove] = useState(false);

  const reload = useCallback(() => {
    trackerApi
      .listCharacters()
      .then(setChars)
      .catch(() => setChars([]));
  }, []);
  useEffect(reload, [reload]);

  // Personagem logado no jogo: entra na lista sozinho e fica selecionado.
  const inGame = useStatus()?.own_character ?? null;
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (!inGame || chars === null || handled.current === inGame.name) return;
    handled.current = inGame.name;
    const known = chars.find((c) => c.name.toLowerCase() === inGame.name.toLowerCase());
    if (known) {
      select(known.id);
    } else {
      trackerApi
        .addCharacter(inGame.name, inGame.class)
        .then((c) => {
          select(c.id);
          reload();
        })
        .catch(() => {});
    }
  }, [inGame, chars, reload]);

  const current = chars?.find((c) => c.id === selected) ?? chars?.[0] ?? null;

  // Busca direta: abre o personagem do resultado, criando-o se precisar.
  async function openFromSearch(hit: Parameters<typeof openHit>[1]) {
    const c = await openHit(chars ?? [], hit);
    setSearching(false);
    select(c.id);
    setTab("profile");
    reload();
  }

  function select(id: number) {
    setSelected(id);
    writeSelected(id);
    setConfirmRemove(false);
  }

  if (chars === null) return null;

  if (chars.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Ache seu personagem</CardTitle>
          <CardDescription>
            Digite o nome e o app busca o perfil no site oficial do Aion 2. Entrar no jogo com o app aberto também
            traz seu personagem sozinho. Cada personagem tem a própria lista de mascotes e de diárias e semanais, que
            zeram sozinhas no reset do servidor.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <OfficialSearch onChoose={openFromSearch} />
          {adding ? (
            <NewCharacterForm
              onCreated={(c) => {
                setAdding(false);
                select(c.id);
                reload();
              }}
              onCancel={() => setAdding(false)}
            />
          ) : (
            <Button variant="link" size="sm" className="self-start px-0" onClick={() => setAdding(true)}>
              Prefiro adicionar à mão
            </Button>
          )}
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        {adding ? (
          <NewCharacterForm
            onCreated={(c) => {
              setAdding(false);
              select(c.id);
              reload();
            }}
            onCancel={() => setAdding(false)}
          />
        ) : (
          <>
            <NativeSelect
              value={current?.id ?? ""}
              onChange={(e) => select(Number(e.target.value))}
              aria-label="Personagem"
            >
              {chars.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                  {c.class ? ` (${CLASS_NAMES[c.class as keyof typeof CLASS_NAMES] ?? c.class})` : ""}
                  {inGame && c.name.toLowerCase() === inGame.name.toLowerCase() ? " · no jogo" : ""}
                </option>
              ))}
            </NativeSelect>
            <Button variant="outline" size="sm" onClick={() => setSearching((v) => !v)}>
              <Search /> Buscar personagem
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setAdding(true)} title="Adicionar sem o site oficial">
              <UserPlus /> Adicionar à mão
            </Button>
            {current &&
              (confirmRemove ? (
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={async () => {
                    await trackerApi.removeCharacter(current.id);
                    setConfirmRemove(false);
                    reload();
                  }}
                >
                  <Trash2 /> Apagar {current.name} e as marcações dele
                </Button>
              ) : (
                <Button variant="ghost" size="sm" onClick={() => setConfirmRemove(true)}>
                  <Trash2 /> Remover
                </Button>
              ))}
          </>
        )}
      </div>

      {searching && !adding && (
        <Card className="gap-3">
          <CardContent>
            <OfficialSearch onChoose={openFromSearch} />
          </CardContent>
        </Card>
      )}

      {current && (
        <Tabs value={tab} onValueChange={setTab}>
          <TabsList>
            <TabsTrigger value="profile">Perfil</TabsTrigger>
            <TabsTrigger value="checklist">Diárias e semanais</TabsTrigger>
            <TabsTrigger value="pets">Mascotes</TabsTrigger>
          </TabsList>
          <TabsContent value="profile">
            <ProfilePanel character={current} onChanged={reload} />
          </TabsContent>
          <TabsContent value="checklist">
            <ChecklistPanel characterId={current.id} />
          </TabsContent>
          <TabsContent value="pets">
            <PetsPanel characterId={current.id} />
          </TabsContent>
        </Tabs>
      )}
    </div>
  );
}
