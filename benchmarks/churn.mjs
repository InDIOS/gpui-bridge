// RAM soak: creates and removes a batch of nodes every 200 ms for N seconds.
//   node benchmarks/churn.mjs <seconds> <copies> [type,type,...]   (all but div and tree by default)
// Prints READY after the first batch and DONE at the end; benchmarks/soak.ps1 samples the host's
// memory in between, which must level off: a steady climb is a leak.
import { readFileSync } from "node:fs";
import { createApp } from "../packages/client-node/dist/index.js";
const seconds = Number(process.argv[2] ?? 60);
const copies = Number(process.argv[3] ?? 5);
const png = readFileSync(new URL("../examples/tray-icon.png", import.meta.url)).toString("base64");
const app = await createApp();
let errors = 0;
const seen = new Set();
app.on("error", (e) => { errors++; const k = `${e.code} ${e.op} ${e.message}`.slice(0, 160); if (!seen.has(k)) { seen.add(k); console.error("ERR", k); } });
const root = app.div({ class: "flex flex-wrap gap-2 p-2 size-full overflow-y-scroll" });
app.window({ title: "churn", width: 1000, height: 800, root });
app.commit();
const opts = [{ value: "a", label: "A" }, { value: "b", label: "B" }];
let cycle = 0;
const makers = {
  input: () => app.input({ value: `v${cycle}`, placeholder: "x" }),
  textarea: () => app.textarea({ value: "t" }),
  numberInput: () => app.numberInput({ value: "1" }),
  select: () => app.select({ options: opts, value: "a" }),
  slider: () => app.slider({ min: 0, max: 10, value: 3 }),
  checkbox: () => app.checkbox({ label: "c", checked: true }),
  datePicker: () => app.datePicker({}),
  timeField: () => app.timeField({}),
  colorPicker: () => app.colorPicker({}),
  combobox: () => app.combobox({ options: opts }),
  otpInput: () => app.otpInput({ length: 4 }),
  calendar: () => app.calendar({}),
  table: () => app.table({ columns: [{ key: "a", name: "A" }], rows: [{ a: "1" }, { a: "2" }] }),
  tree: () => app.tree({ items: [{ id: "r", label: "root", children: [{ id: "k", label: "kid" }] }] }),
  chart: () => app.chart({ data: [{ label: "a", value: 1 }, { label: "b", value: 2 }] }),
  list: () => app.list({}).append(app.listItem({ value: "1" }).append(app.text({ content: "row" }))),
  text: () => app.text({ content: `n${cycle}` }),
  transition: () => app.text({ content: `n${cycle}`, class: "transition duration-300 hover:bg-accent" }),
  image: () => app.image({ data: png, class: "size-8" }),
  icon: () => app.icon({ name: "loader-circle", class: "size-4" }),
  div: () => app.div({ class: "p-1" }),
};
// `tree` is left out by default: gpui-component 0.9.0 never frees a TreeState (its row closure
// holds the state itself), ~2 KB per Tree created and removed. Name it to see that climb.
const only = (process.argv[4] ?? Object.keys(makers).filter((k) => k !== "div" && k !== "tree").join(",")).split(",");
function batch() {
  const nodes = [];
  for (let c = 0; c < copies; c++) for (const name of only) nodes.push(makers[name]());
  for (const n of nodes) root.append(n);
  app.commit();
  return nodes;
}
let live = batch();
console.log("READY");
const end = Date.now() + seconds * 1000;
while (Date.now() < end) {
  await new Promise((r) => setTimeout(r, 200));
  for (const n of live) n.remove();
  app.commit();
  cycle++;
  live = batch();
}
console.log(`DONE cycles=${cycle} errors=${errors}`);
await app.close();
process.exit(0);
