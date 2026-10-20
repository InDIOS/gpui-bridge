// Components from gpui-kit's Basic, Form and Layout groups: an alert, badges, tags, keys, a
// progress bar, a rating, a pagination, a stepper, an accordion, a form of fields, a description
// list, a toolbar, a status bar, a resizable split and a calendar. Everything carries an `a11y`
// prop where its own label is not enough.
// Run: node examples/components.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type NodeType, type UINode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const state = { stars: 3, page: 1, step: "account", day: "2024-08-03", plan: "pro", name: "" };
const summary = app.text({ class: "text-sm text-muted-foreground" });
const show = () => summary.set({ content: JSON.stringify(state) });
show();

// A click keeps running the app: every handler commits once.
const onChange = <T>(apply: (value: T) => void) => ({ value }: { value: T }) => {
  apply(value);
  show();
  app.commit();
};

const stars = app
  .rating({ value: state.stars, max: 5, a11y: { role: "slider", label: "Rating", value: String(state.stars), min: 0, max: 5 } })
  .on("change", onChange<number>((value) => (state.stars = value)));

const pages = app
  .pagination({ page: state.page, total: 12, a11y: { label: "Pages" } })
  .on("change", onChange<number>((value) => (state.page = value)));

const steps = app
  .stepper({
    value: state.step,
    items: [
      { value: "account", label: "Account" },
      { value: "plan", label: "Plan" },
      { value: "pay", label: "Payment" },
    ],
  })
  .on("change", onChange<string>((value) => (state.step = value)));

const accordion = app.accordion({
  multiple: true,
  items: [
    { id: "what", title: "What is gpui-bridge?", content: "A native host driven from your language.", open: true },
    { id: "how", title: "How does it talk?", content: "JSON Lines over stdin and stdout." },
  ],
});

const day = app
  .calendar({ value: state.day, months: 1, weekStartsOn: "monday", a11y: { label: "Start date" } })
  .on("change", onChange<string>((value) => (state.day = value)));

const plan = app
  .radioGroup({
    options: [
      { value: "free", label: "Free" },
      { value: "pro", label: "Pro" },
    ],
    value: state.plan,
  })
  .on("change", onChange<string>((value) => (state.plan = value)));

const name = app
  .input({ placeholder: "Your name", a11y: { role: "textbox", label: "Your name" } })
  .on("change", onChange<string>((value) => (state.name = value)));

const form = app
  .form({ direction: "vertical", class: "w-72" })
  .append(
    app.field({ label: "Plan", description: "You can change it later" }).append(plan),
    app.field({ label: "Name", required: true }).append(name),
  );

const split = app
  .resizable({ axis: "horizontal", sizes: [200], class: "h-48 border border-border rounded-md" })
  .append(
    app
      .div({ class: "flex flex-col gap-1 p-3 size-full", a11y: { role: "region", label: "Sidebar" } })
      .append(app.text({ content: "Sidebar", class: "text-sm font-medium" }), app.tag({ label: "200 px" })),
    app
      .div({ class: "flex flex-col gap-2 p-3 size-full", a11y: { role: "region", label: "Content" } })
      .append(app.text({ content: "Drag the divider" })),
  );

const root = app.div({ class: "flex flex-col size-full gap-3 p-6 bg-background text-foreground" });

const row = (...children: UINode<NodeType>[]) => app.div({ class: "flex items-center gap-3" }).append(...children);

root.append(
  app.alert({
    message: "This build also reports itself to screen readers.",
    title: "Components",
    variant: "info",
    closable: true,
    a11y: { role: "alert", label: "Components" },
  }),
  row(
    app.badge({ count: 3 }).append(app.icon({ name: "bell", class: "size-5" })),
    app.tag({ label: "beta", variant: "success", outline: true }),
    app.kbd({ keys: "ctrl-s" }),
    app.progress({ value: 42, class: "w-40" }),
    app.spinner({ class: "size-4" }),
    app.avatar({ name: "Ada Lovelace" }),
  ),
  row(stars, pages),
  steps,
  accordion,
  row(
    form,
    app.div({ class: "flex flex-col gap-2" }).append(
      app.descriptionList({
        items: [
          { label: "Plan", value: state.plan },
          { label: "Rating", value: String(state.stars) },
          { label: "Day", value: state.day },
        ],
        columns: 2,
        bordered: true,
        class: "w-80",
      }),
      app.groupBox({ title: "Calendar", variant: "outline" }).append(day),
    ),
  ),
  split,
  app.toolbar({ class: "border-b border-border" }).append(
    app.button({ label: "New", variant: "ghost" }),
    app.button({ label: "Open", variant: "ghost" }),
    app.kbd({ keys: "ctrl-o" }),
  ),
  app.statusBar({ left: "Ready", right: "gpui-bridge" }),
  summary,
);
app.window({ title: "Components", width: 900, height: 860, root, theme: "dark" });
app.commit();
