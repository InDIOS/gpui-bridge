// Chat (M15): a MessageScroller with Messages, Bubbles and Markers, an answer streamed into the
// last bubble, an Attachment that fails and is retried, and a Questionnaire beside it. Send a
// message with Enter; "upload" in it attaches a file. The aside shows what the host reports.
// Run: pnpm dev chat   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const scroller = app.messageScroller({ class: "flex-1", jumpLabel: "Jump to latest" });
const status = app.text({ class: "text-sm text-muted-foreground" });
const report = (line: string) => status.set({ content: line });

function message(from: "you" | "bot", text: string) {
  const content = app.text({ content: text });
  const row = app
    .message(from === "you" ? { alignment: "end", a11y: { role: "group", label: "You" } } : { avatarName: "Bot" })
    .append(
      app.messageHeader().append(app.text({ content: from === "you" ? "You" : "Bot" })),
      app.bubble({ variant: from === "you" ? "filled" : "secondary" }).append(content),
    );
  scroller.append(row);
  return { row, content };
}

// An upload that fails at 60% the first time; `retry` starts it again.
function upload(row: ReturnType<typeof message>["row"]) {
  let failedOnce = false;
  const file = app.attachment({ status: "uploading", progress: 0, title: "report.pdf", description: "2.4 MB" });
  const run = () => {
    let progress = 0;
    const timer = setInterval(() => {
      progress += 10;
      if (progress === 60 && !failedOnce) {
        failedOnce = true;
        clearInterval(timer);
        file.set({ status: "failed", progress: null, tooltip: "The connection dropped" });
      } else if (progress === 100) {
        clearInterval(timer);
        file.set({ status: "complete", progress: null, tooltip: null });
      } else file.set({ status: "uploading", progress });
      app.commit();
    }, 200);
  };
  file
    .on("click", () => report("attachment: click"))
    .on("remove", () => {
      file.remove();
      report("attachment: remove");
      app.commit();
    })
    .on("retry", () => {
      report("attachment: retry");
      run();
    });
  row.append(file);
  run();
}

// The answer streams in word by word after a "Thinking" marker.
function answer(question: string) {
  const thinking = app.marker({ content: "Thinking", loading: true, loadingStyle: "shimmer", a11y: { role: "status" } });
  scroller.append(thinking);
  app.commit();
  setTimeout(() => {
    thinking.remove();
    const words = `You said "${question}". This answer arrives a few words at a time, and the list stays at the end while it grows unless you scroll up.`.split(" ");
    const { content } = message("bot", "");
    let shown = 0;
    const timer = setInterval(() => {
      content.set({ content: words.slice(0, ++shown).join(" ") });
      if (shown === words.length) clearInterval(timer);
      app.commit();
    }, 80);
    app.commit();
  }, 800);
}

scroller.append(app.marker({ content: "Today", variant: "separator" }));
message("bot", "Hi! Ask me anything, or type a message with the word upload.");
for (let i = 1; i <= 40; i++) message(i % 2 ? "you" : "bot", `Earlier message ${i}`);

const input = app.input({ placeholder: "Message (Enter to send)", a11y: { label: "Message" } });
input.on("submit", ({ value }) => {
  const text = value.trim();
  if (!text) return;
  const { row } = message("you", text);
  if (/upload/i.test(text)) upload(row);
  input.set({ value: "" });
  app.commit();
  answer(text);
});

const quiz = app
  .questionnaire({
    items: [
      { name: "plan", label: "Which plan do you use?", required: true, choices: [{ value: "free", label: "Free" }, { value: "pro", label: "Pro" }] },
      {
        name: "tools",
        label: "Which clients do you use?",
        multiple: true,
        choices: ["Node", "React", "Python", "Go"].map((label) => ({ value: label.toLowerCase(), label })),
      },
      { name: "notes", label: "Anything else?", input: true },
    ],
  })
  .on("change", ({ value }) => report(`change: ${JSON.stringify(value)}`))
  .on("submit", ({ value }) => {
    report(`submit: ${value.map((a) => `${a.name}=${a.status}`).join(", ")}`);
    scroller.append(app.marker({ content: "Feedback sent", icon: "check" }));
    app.commit();
  });

const chat = app.div({ class: "flex flex-col flex-1 gap-2 p-4" }).append(scroller, input);
const aside = app.div({ class: "flex flex-col gap-4 w-96 p-4 border-l border-border" }).append(quiz, status);
app.window({
  title: "Chat",
  width: 1100,
  height: 720,
  root: app.div({ class: "flex flex-row size-full bg-background text-foreground" }).append(chat, aside),
});
app.commit();
