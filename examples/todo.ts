// Todo list: an Input that adds tasks on submit, a Checkbox per task and a
// button that clears the completed ones, driven entirely from Node.
// Run: node examples/todo.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type UINode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

interface Task {
  node: UINode<"Checkbox">;
  done: boolean;
}
const tasks: Task[] = [];

const root = app.div({ class: "flex flex-col gap-3 p-6 size-full bg-background text-foreground" });
const title = app.text({ content: "Todo", class: "text-xl font-bold" });
const input = app.input({ placeholder: "What needs doing? (Enter to add)" });
const list = app.div({ class: "flex flex-col gap-2" });
const footer = app.div({ class: "flex flex-row items-center justify-between" });
const remaining = app.text({ class: "text-sm text-muted-foreground" });
const clear = app.button({ label: "Clear completed", variant: "secondary" });

// Footer text and button state follow the task list.
function refreshFooter() {
  const left = tasks.filter((task) => !task.done).length;
  remaining.set({ content: `${left} ${left === 1 ? "item" : "items"} left` });
  clear.set({ disabled: !tasks.some((task) => task.done) });
}

input.on("submit", ({ value }) => {
  const label = value.trim();
  if (!label) return;
  // Checkboxes are synced (the host toggles them); `change` keeps our copy in step.
  const task: Task = { node: app.checkbox({ label }), done: false };
  task.node.on("change", ({ checked }) => {
    task.done = checked;
    refreshFooter();
    app.commit();
  });
  tasks.push(task);
  list.append(task.node);
  input.set({ value: "" });
  refreshFooter();
  app.commit();
});

clear.on("click", () => {
  for (const task of tasks.filter((task) => task.done)) task.node.remove();
  tasks.splice(0, tasks.length, ...tasks.filter((task) => !task.done));
  refreshFooter();
  app.commit();
});

footer.append(remaining, clear);
root.append(title, input, list, footer);
refreshFooter();
app.window({ title: "Todo", width: 420, height: 420, root });
app.commit();
