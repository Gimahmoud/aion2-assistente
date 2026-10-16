import { open } from "@tauri-apps/plugin-dialog";
import {
  Activity,
  CalendarClock,
  Contact,
  FileUp,
  History,
  Lock,
  Move,
  PanelTop,
  RotateCcw,
  Stethoscope,
  Users,
} from "lucide-react";
import { useEffect, useState } from "react";

import { CharactersTab } from "@/components/CharactersTab";
import { DiagnosticsTab } from "@/components/DiagnosticsTab";
import { EventsTab } from "@/components/EventsTab";
import { HistoryTab } from "@/components/HistoryTab";
import { MeterTab } from "@/components/MeterTab";
import { PlayersTab } from "@/components/PlayersTab";
import { StatusBanner } from "@/components/StatusBanner";
import { UpdateBanner } from "@/components/UpdateBanner";
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
  const [recording, setRecording] = useState(false);
  const [accepted, setAccepted] = useState(readDisclaimer);

  const overlayLocked = useEvent<boolean>("overlay:click_through", true);

  useEffect(() => setRecording(!!status?.recording), [status?.recording]);

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
    const defaultPath = await api.recordingsDir().catch(() => undefined);
    const path = await open({
      multiple: false,
      defaultPath,
      filters: [{ name: "Captura", extensions: ["pcap"] }],
    });
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

      <UpdateBanner />
      <StatusBanner status={status} />

      <Tabs defaultValue="meter">
        <TabsList>
          <TabsTrigger value="meter">
            <Activity /> Medidor
          </TabsTrigger>
          <TabsTrigger value="history">
            <History /> Histórico
          </TabsTrigger>
          <TabsTrigger value="players">
            <Contact /> Jogadores
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
          <MeterTab />
        </TabsContent>
        <TabsContent value="history">
          <HistoryTab />
        </TabsContent>
        <TabsContent value="players">
          <PlayersTab live={live} />
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
