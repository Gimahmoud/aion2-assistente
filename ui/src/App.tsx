import { open } from "@tauri-apps/plugin-dialog";
import { Activity, CalendarClock, FileUp, History, Lock, Move, PanelTop, RotateCcw, Stethoscope, Users } from "lucide-react";
import { useEffect, useState } from "react";

import { CharactersTab } from "@/components/CharactersTab";
import { DiagnosticsTab } from "@/components/DiagnosticsTab";
import { EncounterDetail } from "@/components/EncounterDetail";
import { EventsTab } from "@/components/EventsTab";
import { HistoryTab } from "@/components/HistoryTab";
import { StatusBanner } from "@/components/StatusBanner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api, useEvent, useStatus } from "@/lib/api";
import type { EncounterSnapshot } from "@/lib/types";

const DISCLAIMER_KEY = "aviso-aceito-v1";

function readDisclaimer() {
  try {
    return localStorage.getItem(DISCLAIMER_KEY) === "1";
  } catch {
    return false;
  }
}

export default function App() {
  const status = useStatus();
  const live = useEvent<EncounterSnapshot | null>("combat:snapshot", null);
  const closed = useEvent<{ encounter: EncounterSnapshot } | null>("combat:closed", null);
  const [recording, setRecording] = useState(false);
  const [accepted, setAccepted] = useState(readDisclaimer);

  const [lastSaved, setLastSaved] = useState<EncounterSnapshot | null>(null);
  const overlayLocked = useEvent<boolean>("overlay:click_through", true);

  useEffect(() => setRecording(!!status?.recording), [status?.recording]);

  // Ao abrir o app, mostra a última luta salva até começar uma nova.
  useEffect(() => {
    api
      .listEncounters(1)
      .then((rows) => (rows[0] ? api.getEncounter(rows[0].id) : null))
      .then(setLastSaved)
      .catch(() => {});
  }, []);

  // Mostra a luta ao vivo; quando ela acaba, mantém a última encerrada na tela.
  const shown = live ?? closed?.encounter ?? lastSaved;

  if (!accepted) {
    return (
      <main className="mx-auto flex min-h-screen max-w-xl items-center p-6">
        <Card>
          <CardHeader>
            <CardTitle>Antes de começar</CardTitle>
            <CardDescription>Aion 2 Assistente é um projeto independente, sem vínculo com a NCSoft.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 text-sm">
            <p>
              O medidor apenas <strong>escuta</strong> uma cópia do tráfego de rede do jogo. Ele não abre o processo do
              jogo, não lê memória e não envia nem altera pacotes.
            </p>
            <p>
              Mesmo assim, a NCSoft nunca autorizou oficialmente medidores de terceiros. O risco de usar é baixo, mas é
              seu.
            </p>
            <p>Tudo fica salvo só no seu computador.</p>
            <Button
              className="self-end"
              onClick={() => {
                try {
                  localStorage.setItem(DISCLAIMER_KEY, "1");
                } catch {
                  /* sem armazenamento: pergunta de novo na próxima vez */
                }
                setAccepted(true);
              }}
            >
              Entendi
            </Button>
          </CardContent>
        </Card>
      </main>
    );
  }

  async function pickReplay() {
    const path = await open({ multiple: false, filters: [{ name: "Captura", extensions: ["pcap"] }] });
    if (typeof path === "string") await api.replayFile(path);
  }

  return (
    <main className="flex min-h-screen flex-col gap-4 p-4">
      <header className="flex flex-wrap items-center gap-2">
        <h1 className="mr-auto text-lg font-semibold">Aion 2 Assistente</h1>
        <label className="text-muted-foreground flex items-center gap-2 text-sm">
          <Switch
            checked={recording}
            onCheckedChange={async (v) => {
              await api.setRecording(v);
              setRecording(v);
            }}
          />
          Gravar tráfego
        </label>
        <Button variant="outline" size="sm" onClick={pickReplay}>
          <FileUp /> Abrir gravação
        </Button>
        <Button variant="outline" size="sm" onClick={() => api.toggleOverlay()}>
          <PanelTop /> Overlay
        </Button>
        <Button
          variant={overlayLocked ? "outline" : "secondary"}
          size="sm"
          onClick={() => api.setOverlayClickThrough(!overlayLocked)}
        >
          {overlayLocked ? (
            <>
              <Move /> Mover overlay
            </>
          ) : (
            <>
              <Lock /> Travar overlay
            </>
          )}
        </Button>
        <Button size="sm" onClick={() => api.resetEncounter()}>
          <RotateCcw /> Nova luta
        </Button>
      </header>

      <StatusBanner status={status} />

      <Tabs defaultValue="meter">
        <TabsList>
          <TabsTrigger value="meter">
            <Activity /> Medidor
          </TabsTrigger>
          <TabsTrigger value="history">
            <History /> Histórico
          </TabsTrigger>
          <TabsTrigger value="characters">
            <Users /> Personagens
          </TabsTrigger>
          <TabsTrigger value="events">
            <CalendarClock /> Eventos
          </TabsTrigger>
          <TabsTrigger value="diag">
            <Stethoscope /> Diagnóstico
          </TabsTrigger>
        </TabsList>
        <TabsContent value="meter">
          {shown ? (
            <EncounterDetail encounter={shown} />
          ) : (
            <p className="text-muted-foreground p-6 text-sm">
              Nenhuma luta ainda. Ataque algo no jogo e o DPS aparece aqui. Atalhos: Ctrl+Shift+R inicia uma nova luta,
              Ctrl+Shift+O destrava o overlay para mover.
            </p>
          )}
        </TabsContent>
        <TabsContent value="history">
          <HistoryTab />
        </TabsContent>
        <TabsContent value="characters">
          <CharactersTab />
        </TabsContent>
        <TabsContent value="events">
          <EventsTab />
        </TabsContent>
        <TabsContent value="diag">
          <DiagnosticsTab status={status} />
        </TabsContent>
      </Tabs>
    </main>
  );
}
