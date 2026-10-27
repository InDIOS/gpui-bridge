// Settings: two pages of fields of the five types; each change shows its key and value.
// Run: node examples/settings.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type SettingsPage } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const pages: SettingsPage[] = [
  {
    title: "General",
    icon: "settings",
    groups: [
      {
        title: "Appearance",
        items: [
          { key: "dark", label: "Dark mode", type: "switch", value: false },
          {
            key: "font",
            label: "Font",
            type: "dropdown",
            value: "mono",
            options: [
              { value: "mono", label: "Monospace" },
              { value: "sans", label: "Sans serif" },
            ],
          },
        ],
      },
      {
        title: "Updates",
        items: [
          { key: "auto", label: "Check automatically", type: "checkbox", value: true },
          { key: "beta", label: "Beta channel", type: "switch", value: false, disabled: true, description: "Coming soon" },
        ],
      },
    ],
  },
  {
    title: "Editor",
    icon: "file",
    groups: [
      {
        items: [
          { key: "name", label: "Project name", type: "input", value: "demo", description: "Shown in the title bar" },
          { key: "size", label: "Font size", type: "number", value: 14, min: 8, max: 32 },
        ],
      },
    ],
  },
];

const root = app.div({ class: "flex flex-col size-full bg-background" });
const label = app.text({ content: "Change a setting", class: "p-3 text-sm text-muted-foreground" });
const settings = app.settings({ pages, class: "flex-1" });

settings.on("change", ({ value }) => {
  label.set({ content: `${value.key} = ${JSON.stringify(value.value)}` });
  app.commit();
});

root.append(label, settings);
app.window({ title: "Settings", width: 760, height: 520, root });
app.commit();
