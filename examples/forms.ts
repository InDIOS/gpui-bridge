// Form controls: Switch, RadioGroup, Select (searchable), Slider and Textarea (M11) plus the
// four controls with state of M14 (DatePicker, TimeField, ColorPicker, Combobox). The summary
// line shows what the host reports; the volume slider and the language list are controlled (the
// app confirms its value).
// Run: pnpm dev forms   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const state: {
  wifi: boolean;
  size: string;
  country: string;
  volume: number;
  notes: string;
  /** `DatePicker` and `ColorPicker` clear to `null`. */
  day: string | null;
  time: string;
  color: string | null;
  tags: string[];
} = {
  wifi: true,
  size: "m",
  country: "",
  volume: 40,
  notes: "",
  day: "2024-08-03",
  time: "09:30",
  color: "#3366ff",
  tags: ["rs"],
};
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
const volume = app.slider({ min: 0, max: 100, step: 10, value: state.volume, controlled: true, class: "w-64", a11y: { label: "Volume" } });
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

// M14: form controls with state. Each keeps its value in the host and reports `change`; the
// summary line shows the value the host keeps. `tags` is controlled: the app keeps at most two
// and confirms them, so the host shows only what the app accepted.
const day = app
  .datePicker({ value: state.day, placeholder: "Pick a day", a11y: { label: "Pick a day" } })
  .on("change", ({ value }) => {
    state.day = value;
    show();
    app.commit();
  });

const time = app.timeField({ value: state.time, a11y: { label: "Time" } }).on("change", ({ value }) => {
  state.time = value;
  show();
  app.commit();
});

const color = app
  .colorPicker({ value: state.color, a11y: { label: "Color" } })
  .on("change", ({ value }) => {
    state.color = value;
    show();
    app.commit();
  });

const tags = app
  .combobox({
    options: [
      { value: "rs", label: "Rust" },
      { value: "go", label: "Go" },
      { value: "ts", label: "TypeScript" },
      { value: "py", label: "Python", disabled: true },
    ],
    value: state.tags,
    placeholder: "Languages",
    searchable: true,
    controlled: true,
    class: "w-64",
    a11y: { label: "Languages" },
  })
  .on("change", ({ value }) => {
    // Controlled: the app keeps at most two languages and confirms that.
    state.tags = value.slice(0, 2);
    tags.set({ value: state.tags });
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
  field("Date", day),
  field("Time", time),
  field("Color", color),
  field("Languages", tags),
  summary,
);
app.window({ title: "Forms", width: 620, height: 700, root });
app.commit();
