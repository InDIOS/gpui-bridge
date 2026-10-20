// Several windows: the main window opens a settings window. The theme is app-wide, so the
// settings window's switch repaints both. Closing the main window ends the app only when
// it is the last one open. Both windows open with their field focused, from a `focus()` in the
// commit that opens the window.
// Run: node examples/windows.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type AppWindow } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const main = app.div({ class: "flex flex-col gap-3 p-6 size-full bg-background text-foreground" });
const status = app.text({ content: "Settings closed", class: "text-sm text-muted-foreground" });
const note = app.input({ placeholder: "Type in the main window" });
const open = app.button({ label: "Open settings", variant: "primary" });
main.append(app.text({ content: "Main window", class: "text-lg font-bold" }), note, open, status);

let settings: AppWindow | undefined;
let dark = false;

function openSettings() {
  const root = app.div({ class: "flex flex-col gap-3 p-6 size-full bg-background text-foreground" });
  const name = app.input({ placeholder: "Type in the settings window" });
  const darkMode = app.checkbox({ label: "Dark mode", checked: dark });
  const close = app.button({ label: "Close settings" });
  root.append(app.text({ content: "Settings", class: "text-lg font-bold" }), name, darkMode, close);

  darkMode.on("change", ({ checked }) => {
    dark = checked;
    app.window({ theme: dark ? "dark" : "light" });
    app.commit();
  });
  close.on("click", () => {
    // Closed by the app: no "closed" event, so update the state here.
    settings?.close();
    settings = undefined;
    root.remove();
    status.set({ content: "Settings closed by the app" });
    open.set({ disabled: false });
    app.commit();
  });

  const window = app.createWindow({ title: "App settings", width: 360, height: 260, root });
  window.on("closed", () => {
    settings = undefined;
    root.remove();
    status.set({ content: "Settings closed by the user" });
    open.set({ disabled: false });
    app.commit();
  });
  settings = window;
  status.set({ content: "Settings open" });
  open.set({ disabled: true });
  // Focused in the same commit that opens the window: the host applies it after the window
  // exists, so the field is ready to type in without clicking.
  name.focus();
  app.commit();
}

open.on("click", openSettings);
app.on("windowClosed", ({ id }) => console.log(`windowClosed ${id}`));
// Same idea in the commit that opens the first window.
note.focus();
app.window({ title: "Main", width: 420, height: 300, root: main });
app.commit();
