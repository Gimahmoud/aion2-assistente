import { invoke } from "@tauri-apps/api/core";
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { useEvent } from "@/lib/api";

/** Aviso enquanto o app baixa e instala uma versão nova sozinho. */
export function UpdateBanner() {
  const [initial, setInitial] = useState<string | null>(null);
  const live = useEvent<string | null>("update:installing", null);
  useEffect(() => {
    invoke<string | null>("update_installing").then(setInitial).catch(() => {});
  }, []);
  const version = live ?? initial;
  if (!version) return null;
  return (
    <Alert>
      <Loader2 className="animate-spin" />
      <AlertTitle>Atualizando para a versão {version}</AlertTitle>
      <AlertDescription>O app vai fechar e abrir de novo sozinho em instantes.</AlertDescription>
    </Alert>
  );
}
