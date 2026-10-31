// List (M15): 10,000 contacts with search and the keyboard, a second list that loads 50 rows at a
// time as it is scrolled, and one row whose content is 40 nested nodes (the stack test). Type to
// filter, move with the arrows, Enter confirms. The bar at the bottom shows what the host reports.
// Run: pnpm dev list   (after `pnpm bootstrap`)

import { createApp, type NodeType, type UINode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const status = app.text({ class: "text-sm text-muted-foreground", content: "Arrows move, Enter confirms, Escape clears." });
const report = (line: string) => {
  console.log(line);
  status.set({ content: line });
  app.commit();
};

const NAMES = ["Ada", "Alan", "Grace", "Edsger", "Barbara", "Donald", "Frances", "Ken", "Margaret", "Linus"];

// Every row is created once; a search moves the rows that do not match into `hidden`, a node
// outside the window, and back.
const hidden = app.div();
const contacts = Array.from({ length: 10_000 }, (_, ix) => {
  const name = `${NAMES[ix % NAMES.length]} ${ix + 1}`;
  const value = `c${ix + 1}`;
  // A few rows are disabled: the arrows pass over them.
  const item = app.listItem({ value, disabled: ix % 97 === 96, a11y: { label: name } }).append(
    app.icon({ name: "user", class: "size-4" }),
    app.text({ content: name, class: "flex-1" }),
    app.text({ content: `#${ix + 1}`, class: "text-xs text-muted-foreground" }),
  );
  return { name, value, item };
});

const people = app
  .list({ class: "flex-1", searchable: true, placeholder: "Search 10,000 contacts", value: "c1" })
  .on("change", ({ value }) => report(`contacts: change ${value}`))
  .on("confirm", ({ value }) => report(`contacts: confirm ${value}`))
  .on("query", ({ value }) => {
    const query = value.toLowerCase();
    const found = contacts.filter((contact) => contact.name.toLowerCase().includes(query));
    const shown = new Set(found);
    for (const contact of contacts) (shown.has(contact) ? people : hidden).append(contact.item);
    // Enter confirms the first match.
    people.set({ value: found[0]?.value ?? null });
    report(`contacts: query "${value}", ${found.length} rows`);
    app.commit();
  });
people.append(...contacts.map((contact) => contact.item));

// The stack test: a row whose content is 40 nested `div`s.
let deep: UINode<NodeType> = app.text({ content: "40 levels deep" });
for (let level = 0; level < 40; level++) deep = app.div({ class: "flex" }).append(deep);
people.insertBefore(app.listItem({ value: "deep" }).append(app.icon({ name: "star", class: "size-4" }), deep), contacts[0].item);

// Loaded 50 rows at a time, up to 500; the first page waits a second behind the skeleton.
let loaded = 0;
const feed = app
  .list({ class: "flex-1", loading: true, hasMore: true, loadMoreThreshold: 10 })
  .on("change", ({ value }) => report(`feed: change ${value}`))
  .on("confirm", ({ value }) => report(`feed: confirm ${value}`))
  .on("loadMore", () => {
    report(`feed: loadMore after ${loaded} rows`);
    setTimeout(page, 300);
  });
function page() {
  for (let n = 0; n < 50; n++) {
    loaded++;
    feed.append(app.listItem({ value: `f${loaded}`, a11y: { label: `Event ${loaded}` } }).append(app.text({ content: `Event ${loaded}` })));
  }
  feed.set({ loading: false, hasMore: loaded < 500 });
  app.commit();
}
setTimeout(page, 1000);

app.window({
  title: "List",
  width: 820,
  height: 620,
  root: app.div({ class: "size-full flex flex-col gap-3 p-4" }).append(
    app.div({ class: "flex-1 flex gap-4 min-h-0" }).append(
      app.div({ class: "flex-1 flex flex-col border rounded-md overflow-hidden" }).append(people),
      app.div({ class: "w-64 flex flex-col border rounded-md overflow-hidden" }).append(feed),
    ),
    status,
  ),
});
people.focus();
app.commit();
