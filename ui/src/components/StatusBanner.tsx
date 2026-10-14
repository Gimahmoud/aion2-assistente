import { openUrl } from "@tauri-apps/plugin-opener";
import { AlertTriangle, Download, Loader2, Radio, Search } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import type { Status } from "@/lib/types";

/** Explica o que o medidor está fazendo e o que o usuário precisa fazer. */
export function StatusBanner({ status }: { status: Status | null }) {
  if (!status) return null;
  switch (status.state) {
    case "npcap_missing":
      return (
        <Alert variant="destructive">
          <AlertTriangle />
          <AlertTitle>Npcap não instalado</AlertTitle>
          <AlertDescription>
            <p>
              O medidor precisa do Npcap para ler o tráfego de rede. Na instalação, marque “WinPcap API-compatible
              Mode” e deixe desmarcado “Restrict Npcap driver's access to Administrators only”. Depois reinicie o app.
            </p>
            <Button size="sm" variant="outline" onClick={() => openUrl(status.npcap_download)}>
              <Download /> Baixar Npcap
            </Button>
          </AlertDescription>
        </Alert>
      );
    case "waiting_for_game":
      return (
        <Alert>
          <Search />
          <AlertTitle>Aguardando o Aion 2</AlertTitle>
          <AlertDescription>{status.message}</AlertDescription>
        </Alert>
      );
    case "error":
      return (
        <Alert variant="destructive">
          <AlertTriangle />
          <AlertTitle>Erro na captura</AlertTitle>
          <AlertDescription>{status.message}</AlertDescription>
        </Alert>
      );
    case "replaying":
      return (
        <Alert>
          <Loader2 className="animate-spin" />
          <AlertTitle>Replay</AlertTitle>
          <AlertDescription>{status.message}</AlertDescription>
        </Alert>
      );
    case "capturing":
      return (
        <Alert>
          <Radio />
          <AlertTitle>Capturando</AlertTitle>
          <AlertDescription>{status.message}</AlertDescription>
        </Alert>
      );
    default:
      return null;
  }
}
