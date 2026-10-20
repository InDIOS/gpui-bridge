// System actions: open and save dialogs, clipboard, opening a URL or a file, and a drop zone
// for files from the Explorer.
// Run: node examples/system.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

let last: string | null = null;
const status = app.text({ content: "Pick a file, or drop files on the box.", class: "text-sm text-muted-foreground whitespace-normal" });
const say = (content: string) => {
  status.set({ content });
  app.commit();
};

const button = (label: string, run: () => Promise<void> | void) =>
  app.button({ label }).on("click", () => void Promise.resolve(run()).catch((err) => say(String(err))));

const actions = app.div({ class: "flex flex-row flex-wrap gap-2" }).append(
  button("Open files…", async () => {
    const paths = await app.openDialog({ multiple: true, prompt: "Pick" });
    last = paths?.[0] ?? last;
    say(paths ? `Opened: ${paths.join(", ")}` : "Open cancelled");
  }),
  button("Choose folder…", async () => {
    const paths = await app.openDialog({ files: false, directories: true });
    say(paths ? `Folder: ${paths[0]}` : "Cancelled");
  }),
  button("Save as…", async () => {
    const path = await app.saveDialog({ name: "notes.txt" });
    say(path ? `Save to: ${path}` : "Save cancelled");
  }),
  button("Copy path", () => {
    app.writeClipboard(last ?? "nothing picked yet");
    say("Copied to the clipboard");
  }),
  button("Paste", async () => say(`Clipboard: ${(await app.readClipboard()) ?? "(no text)"}`)),
  button("Open picked file", () => (last ? app.openPath(last) : say("Pick a file first"))),
  button("Open website", () => app.openUrl("https://github.com/zed-industries/zed")),
);

const drop = app
  .div({ class: "flex items-center justify-center h-32 rounded-lg border border-border bg-muted" })
  .append(app.text({ content: "Drop files here", class: "text-sm" }))
  .on("drop", ({ paths }) => {
    last = paths[0] ?? last;
    say(`Dropped: ${paths.join(", ")}`);
  });

const root = app.div({ class: "flex flex-col size-full gap-4 p-6 bg-background text-foreground" }).append(actions, drop, status);
app.window({ title: "System", width: 620, height: 380, root });
app.commit();
