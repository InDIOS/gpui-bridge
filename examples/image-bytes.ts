// Image bytes: the client reads a file (a remote image would be downloaded the same way) and
// sends the bytes as base64 in `Image.data`, which the host draws like any other image.
// Run: node examples/image-bytes.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });
const file = fileURLToPath(new URL("tray-icon.png", import.meta.url));
const bytes = () => readFileSync(file).toString("base64");

let sends = 0;
const root = app.div({ class: "flex flex-col size-full items-center justify-center gap-4 p-6 bg-background text-foreground" });
const status = app.text({ content: "The same file twice: a path on the left, bytes on the right.", class: "text-sm text-muted-foreground" });
const fromPath = app.image({ src: file, fit: "contain", class: "size-28 rounded-lg border border-border" });
const fromBytes = app.image({ data: bytes(), fit: "contain", class: "size-28 rounded-lg border border-border" });

const column = (label: string, node: ReturnType<typeof app.image>) =>
  app.div({ class: "flex flex-col items-center gap-2" }).append(node, app.text({ content: label, class: "text-xs text-muted-foreground" }));
const row = app.div({ class: "flex flex-row items-center gap-8" }).append(column("src (a path)", fromPath), column("data (base64 bytes)", fromBytes));

const reload = app.button({ label: "Send the bytes again", variant: "secondary" });
reload.on("click", () => {
  // Sending `data` again, even with the same bytes, makes the host read them again.
  fromBytes.set({ data: bytes() });
  status.set({ content: `Bytes sent ${++sends} more time(s).` });
  app.commit();
});

root.append(
  app.text({ content: "Image from bytes", class: "text-lg" }),
  status,
  row,
  reload,
);
app.window({ title: "Image from bytes", width: 520, height: 360, root });
app.commit();
