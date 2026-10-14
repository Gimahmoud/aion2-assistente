import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { fmtNumber } from "@/lib/format";
import type { Status } from "@/lib/types";

/** Números para depurar a captura e o parser (útil a cada patch do jogo). */
export function DiagnosticsTab({ status }: { status: Status | null }) {
  if (!status) return null;
  const s = status.stats;
  const rows: [string, string][] = [
    ["Estado", status.state],
    ["Interface", status.device ?? "-"],
    ["Servidores do jogo", status.servers.join(", ") || "-"],
    ["Gravando em", status.recording ?? "não"],
    ["Tabela de opcodes", s.opcode_table],
    ["Segmentos TCP", fmtNumber(s.segments)],
    ["Bytes", fmtNumber(s.bytes)],
    ["Mensagens", fmtNumber(s.messages)],
    ["Lotes comprimidos", fmtNumber(s.compressed_batches)],
    ["Ressincronizações", fmtNumber(s.resyncs)],
    ["Buracos TCP", fmtNumber(s.tcp_gaps)],
    ["Erros de descompressão", fmtNumber(s.decompress_errors)],
    ["Segmentos descartados", fmtNumber(status.dropped_segments)],
  ];
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card className="gap-2">
        <CardHeader>
          <CardTitle>Captura</CardTitle>
          <CardDescription>{status.message}</CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableBody>
              {rows.map(([k, v]) => (
                <TableRow key={k}>
                  <TableCell className="text-muted-foreground">{k}</TableCell>
                  <TableCell className="font-mono text-xs break-all whitespace-normal">{v}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <Card className="gap-2">
        <CardHeader>
          <CardTitle>Eventos decodificados</CardTitle>
          <CardDescription>Opcodes sem decodificador aparecem abaixo para estudo.</CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableBody>
              {Object.entries(s.events).map(([k, v]) => (
                <TableRow key={k}>
                  <TableCell>{k}</TableCell>
                  <TableCell className="text-right tabular-nums">{fmtNumber(v)}</TableCell>
                </TableRow>
              ))}
              {s.unknown_opcodes.map(([op, n]) => (
                <TableRow key={op}>
                  <TableCell className="text-muted-foreground font-mono text-xs">desconhecido {op}</TableCell>
                  <TableCell className="text-muted-foreground text-right tabular-nums">{fmtNumber(n)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
