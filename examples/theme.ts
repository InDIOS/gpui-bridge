// Theme: switches between the default theme and a custom theme file, in light and dark.
// Run: node examples/theme.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type ThemeSet, type WindowTheme } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

// A gpui-component theme file; colors not listed keep the default theme's.
const brand: ThemeSet = {
  name: "Brand",
  themes: [
    {
      name: "Brand Light",
      mode: "light",
      radius: 12,
      colors: {
        background: "#fff7ed",
        foreground: "#431407",
        border: "#fed7aa",
        "primary.background": "#ea580c",
        "primary.foreground": "#ffffff",
        "secondary.background": "#ffedd5",
        "secondary.foreground": "#9a3412",
        "muted.background": "#ffedd5",
        "muted.foreground": "#9a3412",
      },
    },
    {
      name: "Brand Dark",
      mode: "dark",
      radius: 12,
      colors: {
        background: "#1c0a00",
        foreground: "#ffedd5",
        border: "#7c2d12",
        "primary.background": "#f97316",
        "primary.foreground": "#1c0a00",
        "secondary.background": "#431407",
        "secondary.foreground": "#fed7aa",
        "muted.background": "#431407",
        "muted.foreground": "#fdba74",
      },
    },
  ],
};
const defaults: ThemeSet = { themes: [] };

let custom = false;
let mode: WindowTheme = "light";

const root = app.div({ class: "flex flex-col gap-4 p-6 size-full bg-background" });
const title = app.text({ content: "", class: "text-lg font-bold text-foreground" });

const card = app.div({ class: "flex flex-col gap-3 p-4 border border-border rounded-lg bg-muted" });
const hint = app.text({ content: "Components and classes follow the theme.", class: "text-sm text-muted-foreground" });
const input = app.input({ placeholder: "Type something" });
const checkbox = app.checkbox({ label: "A checkbox", checked: true });
const badges = app.div({ class: "flex flex-row gap-2" });
for (const role of ["success", "warning", "danger", "info"]) {
  const badge = app.div({ class: `px-2 py-1 rounded-md bg-${role}` });
  badge.append(app.text({ content: role, class: `text-sm text-${role}-foreground` }));
  badges.append(badge);
}
card.append(hint, input, checkbox, badges);

const buttons = app.div({ class: "flex flex-row gap-2" });
const toggleTheme = app.button({ label: "", variant: "primary" });
const toggleMode = app.button({ label: "", variant: "secondary" });
buttons.append(toggleTheme, toggleMode);

function update() {
  title.set({ content: `${custom ? "Brand" : "Default"} theme, ${mode}` });
  toggleTheme.set({ label: custom ? "Use default theme" : "Use brand theme" });
  toggleMode.set({ label: mode === "light" ? "Dark mode" : "Light mode" });
}

toggleTheme.on("click", () => {
  custom = !custom;
  app.window({ themeSet: custom ? brand : defaults });
  update();
  app.commit();
});
toggleMode.on("click", () => {
  mode = mode === "light" ? "dark" : "light";
  app.window({ theme: mode });
  update();
  app.commit();
});

root.append(title, card, buttons);
update();
app.window({ title: "Theme", width: 420, height: 360, root });
app.commit();
