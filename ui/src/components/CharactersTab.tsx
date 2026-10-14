import { Plus, Trash2, UserPlus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { ChecklistPanel } from "@/components/ChecklistPanel";
import { PetsPanel } from "@/components/PetsPanel";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { CLASS_NAMES } from "@/lib/format";
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
  const [confirmRemove, setConfirmRemove] = useState(false);

  const reload = useCallback(() => {
    trackerApi
      .listCharacters()
      .then(setChars)
      .catch(() => setChars([]));
  }, []);
  useEffect(reload, [reload]);

  const current = chars?.find((c) => c.id === selected) ?? chars?.[0] ?? null;

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
          <CardTitle>Adicione seu primeiro personagem</CardTitle>
          <CardDescription>
            Cada personagem tem a própria lista de mascotes e de diárias e semanais. As contagens zeram sozinhas no
            reset do servidor.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <NewCharacterForm
            onCreated={(c) => {
              select(c.id);
              reload();
            }}
          />
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
                </option>
              ))}
            </NativeSelect>
            <Button variant="outline" size="sm" onClick={() => setAdding(true)}>
              <UserPlus /> Novo personagem
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

      {current && (
        <Tabs defaultValue="checklist">
          <TabsList>
            <TabsTrigger value="checklist">Diárias e semanais</TabsTrigger>
            <TabsTrigger value="pets">Mascotes</TabsTrigger>
          </TabsList>
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
