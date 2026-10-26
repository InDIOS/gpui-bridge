// Form controls: Switch, RadioGroup, Select (searchable), Slider and Textarea. The summary
// line shows what the host reports; the volume slider is controlled (the app rounds it).
// Run: node examples/forms.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const state = { wifi: true, size: "m", country: "", volume: 40, notes: "" };
const summary = app.text({ class: "text-sm text-muted-foreground" });
const show = () => summary.set({ content: JSON.stringify(state) });
show();

const field = (label: string, ...children: Parameters<ReturnType<typeof app.div>["append"]>) =>
  app.div({ class: "flex flex-col gap-1" }).append(app.text({ content: label, class: "text-sm font-medium" }), ...children);

const wifi = app.switch({ label: "Wi-Fi", checked: state.wifi }).on("change", ({ checked }) => {
  state.wifi = checked;
  show();
  app.commit();
});

const size = app
  .radioGroup({
    options: [
      { value: "s", label: "Small" },
      { value: "m", label: "Medium" },
      { value: "l", label: "Large" },
      { value: "xl", label: "Extra large (sold out)", disabled: true },
    ],
    value: state.size,
    direction: "horizontal",
    a11y: { role: "group", label: "Size" },
  })
  .on("change", ({ value }) => {
    state.size = value;
    show();
    app.commit();
  });

const country = app
  .select({
    options: ["Argentina", "Brazil", "Chile", "Paraguay", "Uruguay"].map((name) => ({ value: name.slice(0, 2).toLowerCase(), label: name })),
    placeholder: "Choose a country",
    searchable: true,
    class: "w-64",
    a11y: { role: "group", label: "Country" },
  })
  .on("change", ({ value }) => {
    state.country = value;
    show();
    app.commit();
  });

const volumeLabel = app.text({ content: `Volume: ${state.volume}`, class: "text-sm font-medium" });
const volume = app.slider({ min: 0, max: 100, step: 1, value: state.volume, controlled: true, class: "w-64", a11y: { role: "group", label: "Volume" } });
volume.on("change", ({ value }) => {
  // Controlled: the app snaps it to multiples of 10 and confirms that.
  state.volume = Math.round(value / 10) * 10;
  volume.set({ value: state.volume });
  volumeLabel.set({ content: `Volume: ${state.volume}` });
  show();
  app.commit();
});

const notes = app.textarea({ placeholder: "Notes (several lines)", class: "w-96 h-24" }).on("change", ({ value }) => {
  state.notes = value;
  show();
  app.commit();
});

const root = app.div({ class: "flex flex-col size-full gap-4 p-6 bg-background text-foreground" });
root.append(
  wifi,
  field("Size", size),
  field("Country", country),
  app.div({ class: "flex flex-col gap-1" }).append(volumeLabel, volume),
  field("Notes", notes),
  summary,
);
app.window({ title: "Forms", width: 620, height: 560, root });
app.commit();
