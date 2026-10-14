import { Check, Minus, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { cn } from "@/lib/utils";
import {
  fmtCountdown,
  fmtWeekdayTime,
  trackerApi,
  useNow,
  type Checklist,
  type Period,
  type TaskStatus,
} from "@/lib/tracker";

function TaskRow({ task, onSet, onRemove }: { task: TaskStatus; onSet: (n: number) => void; onRemove: () => void }) {
  const done = task.count >= task.meta;
  return (
    <li className="flex items-center gap-3 py-1.5">
      {task.meta === 1 ? (
        <Checkbox checked={done} onCheckedChange={(v) => onSet(v === true ? 1 : 0)} aria-label={task.nome} />
      ) : (
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="icon"
            className="size-7"
            disabled={task.count === 0}
            onClick={() => onSet(task.count - 1)}
            aria-label="Menos um"
          >
            <Minus />
          </Button>
          <span className="w-14 text-center text-sm tabular-nums">
            {task.count}/{task.meta}
          </span>
          <Button
            variant="outline"
            size="icon"
            className="size-7"
            disabled={done}
            onClick={() => onSet(task.count + 1)}
            aria-label="Mais um"
          >
            <Plus />
          </Button>
        </div>
      )}
      <span className={cn("flex-1 text-sm", done && "text-muted-foreground line-through")}>{task.nome}</span>
      {task.escopo === "servidor" && (
        <Badge variant="outline" title="O limite é dividido entre os personagens do mesmo servidor">
          servidor
        </Badge>
      )}
      {task.meta > 1 && !done && (
        <Button variant="ghost" size="sm" onClick={() => onSet(task.meta)}>
          <Check /> Completo
        </Button>
      )}
      {task.custom && (
        <Button variant="ghost" size="icon" className="size-7" onClick={onRemove} aria-label="Remover tarefa">
          <Trash2 />
        </Button>
      )}
    </li>
  );
}

function AddTaskForm({ onAdded }: { onAdded: () => void }) {
  const [name, setName] = useState("");
  const [period, setPeriod] = useState<Period>("diario");
  const [goal, setGoal] = useState(1);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    await trackerApi.addCustomTask(name, period, goal);
    setName("");
    setGoal(1);
    onAdded();
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
      <Input className="w-64" placeholder="Nova tarefa" value={name} onChange={(e) => setName(e.target.value)} />
      <NativeSelect value={period} onChange={(e) => setPeriod(e.target.value as Period)} aria-label="Período">
        <option value="diario">Diária</option>
        <option value="semanal">Semanal</option>
      </NativeSelect>
      <Input
        className="w-20"
        type="number"
        min={1}
        max={999}
        value={goal}
        onChange={(e) => setGoal(Math.max(1, Number(e.target.value) || 1))}
        aria-label="Vezes por período"
        title="Vezes por período"
      />
      <Button type="submit" size="sm" variant="outline" disabled={!name.trim()}>
        <Plus /> Adicionar tarefa
      </Button>
    </form>
  );
}

export function ChecklistPanel({ characterId }: { characterId: number }) {
  const [data, setData] = useState<Checklist | null>(null);
  const now = useNow(1000);

  const reload = useCallback(() => {
    trackerApi
      .getChecklist(characterId)
      .then(setData)
      .catch(() => {});
  }, [characterId]);
  useEffect(reload, [reload]);

  // Recarrega quando passa um reset, para as contagens zerarem na tela.
  const resetPassed = !!data && now >= Math.min(data.next_daily_reset_ms, data.next_weekly_reset_ms);
  useEffect(() => {
    if (resetPassed) reload();
  }, [resetPassed, reload]);

  if (!data) return null;

  async function setCount(task: TaskStatus, n: number) {
    await trackerApi.setTaskCount(characterId, task.id, Math.max(0, Math.min(task.meta, n)));
    reload();
  }

  const sections: { period: Period; title: string; next: number }[] = [
    { period: "diario", title: "Diárias", next: data.next_daily_reset_ms },
    { period: "semanal", title: "Semanais", next: data.next_weekly_reset_ms },
  ];

  return (
    <div className="flex flex-col gap-4">
      {sections.map((s) => {
        const tasks = data.tasks.filter((t) => t.periodo === s.period);
        const done = tasks.filter((t) => t.count >= t.meta).length;
        return (
          <Card key={s.period} className="gap-2">
            <CardHeader>
              <CardTitle>
                {s.title}{" "}
                <span className="text-muted-foreground text-sm font-normal">
                  {done} de {tasks.length} completas
                </span>
              </CardTitle>
              <CardDescription>
                Zera em {fmtCountdown(s.next - now)} ({fmtWeekdayTime(s.next)})
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ul className="divide-y">
                {tasks.map((t) => (
                  <TaskRow
                    key={t.id}
                    task={t}
                    onSet={(n) => setCount(t, n)}
                    onRemove={() => trackerApi.removeCustomTask(t.id).then(reload)}
                  />
                ))}
              </ul>
            </CardContent>
          </Card>
        );
      })}
      <AddTaskForm onAdded={reload} />
    </div>
  );
}
