// Todo list written as a React component with hooks, rendered natively by gpui-bridge.
// Same behavior as examples/todo.ts, but the input and checkboxes are controlled by React state.
// Run: pnpm dev react/todo   (from the repository root)

import { useState } from "react";
import { Button, Checkbox, Div, Input, render, Text } from "gpui-bridge-react";

interface Task {
  id: number;
  label: string;
  done: boolean;
}

let nextId = 1;

function TodoApp() {
  const [draft, setDraft] = useState("");
  const [tasks, setTasks] = useState<Task[]>([]);
  const left = tasks.filter((task) => !task.done).length;

  function add(value: string) {
    const label = value.trim();
    if (!label) return;
    setTasks((tasks) => [...tasks, { id: nextId++, label, done: false }]);
    setDraft("");
  }

  function toggle(id: number, done: boolean) {
    setTasks((tasks) => tasks.map((task) => (task.id === id ? { ...task, done } : task)));
  }

  return (
    <Div class="flex flex-col gap-3 p-6 size-full bg-background text-foreground">
      <Text class="text-xl font-bold">Todo (React)</Text>
      <Input value={draft} placeholder="What needs doing? (Enter to add)" onChange={setDraft} onSubmit={add} />
      <Div class="flex flex-col gap-2">
        {tasks.map((task) => (
          <Checkbox key={task.id} label={task.label} checked={task.done} onChange={(done) => toggle(task.id, done)} />
        ))}
      </Div>
      <Div class="flex flex-row items-center justify-between">
        <Text class="text-sm text-muted-foreground">
          {left} {left === 1 ? "item" : "items"} left
        </Text>
        <Button
          variant="secondary"
          disabled={!tasks.some((task) => task.done)}
          onClick={() => setTasks((tasks) => tasks.filter((task) => !task.done))}
        >
          Clear completed
        </Button>
      </Div>
    </Div>
  );
}

await render(<TodoApp />, {
  window: { title: "Todo (React)", width: 420, height: 420 },
  debug: process.argv.includes("--debug"),
});
