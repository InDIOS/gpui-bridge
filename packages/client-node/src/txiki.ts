// txiki.js entry: the same client API as index.ts, over a `tjs.spawn` transport.
// Uses only txiki.js and Web APIs (no Node modules).

import { connect, type App, type HostEvents, type HostExit, type HostTransport } from "./app.js";
import type { ClientMessage } from "./protocol.js";

export * from "./app.js";
export * from "./protocol.js";

// The subset of txiki.js used here (see types/src/txikijs.d.ts in txiki.js v26.6.0).
interface TjsProcess {
  stdin: WritableStream<Uint8Array>;
  stdout: ReadableStream<Uint8Array>;
  stderr: ReadableStream<Uint8Array>;
  wait(): Promise<{ exit_status: number; term_signal: string | null }>;
  kill(): void;
}
declare const tjs: {
  spawn(args: string[], options: { stdin: "pipe"; stdout: "pipe"; stderr: "pipe" }): TjsProcess;
  env: Record<string, string | undefined>;
  exePath: string;
};

const STDERR_TAIL_LINES = 20;

async function readLines(stream: ReadableStream<Uint8Array>, onLine: (line: string) => void): Promise<void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let newline;
    while ((newline = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newline).replace(/\r$/, "");
      buffer = buffer.slice(newline + 1);
      if (line) onLine(line);
    }
  }
}

class TxikiHost implements HostTransport {
  private process?: TjsProcess;
  private writer?: WritableStreamDefaultWriter<Uint8Array>;
  private readonly encoder = new TextEncoder();
  private exited = false;

  constructor(path: string, args: string[], debug: boolean, events: HostEvents) {
    const stderrTail: string[] = [];
    const finish = (exit: Omit<HostExit, "stderrTail">) => {
      if (this.exited) return;
      this.exited = true;
      events.onExit({ ...exit, stderrTail });
    };
    try {
      this.process = tjs.spawn([path, ...args], { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
    } catch (error) {
      // Reported asynchronously, like a failed spawn in Node.
      queueMicrotask(() => finish({ code: null, signal: null, spawnError: error as Error }));
      return;
    }
    this.writer = this.process.stdin.getWriter();

    const stdout = readLines(this.process.stdout, (line) => {
      try {
        events.onMessage(JSON.parse(line));
      } catch {
        events.onCorruptLine(line);
      }
    });
    const stderr = readLines(this.process.stderr, (line) => {
      stderrTail.push(line);
      if (stderrTail.length > STDERR_TAIL_LINES) stderrTail.shift();
      if (debug) console.error(`[host] ${line}`);
    });
    // Wait for both streams so every line is delivered before the exit.
    Promise.all([this.process.wait(), stdout, stderr]).then(([status]) =>
      finish({ code: status.exit_status, signal: status.term_signal }),
    );
  }

  send(message: ClientMessage): void {
    if (this.exited || !this.writer) return;
    // Writes racing a dying host fail; the exit report carries the real cause.
    this.writer.write(this.encoder.encode(JSON.stringify(message) + "\n")).catch(() => {});
  }

  close(): void {
    this.writer?.close().catch(() => {});
  }

  kill(): void {
    this.process?.kill();
  }
}

export interface TxikiAppOptions {
  /** Host binary. Defaults to `$GPUI_BRIDGE_HOST`, then `gpui-bridge-host[.exe]` next to the running executable. */
  hostPath?: string;
  debug?: boolean;
  headless?: boolean;
}

function defaultHostPath(): string {
  if (tjs.env.GPUI_BRIDGE_HOST) return tjs.env.GPUI_BRIDGE_HOST;
  // A compiled app ships the host next to its own executable.
  const exe = tjs.exePath;
  const name = exe.toLowerCase().endsWith(".exe") ? "gpui-bridge-host.exe" : "gpui-bridge-host";
  return exe.replace(/[\\/][^\\/]*$/, "") + (exe.includes("\\") ? "\\" : "/") + name;
}

export function createTxikiApp(options: TxikiAppOptions = {}): Promise<App> {
  const path = options.hostPath ?? defaultHostPath();
  const args = options.headless ? ["--no-window"] : [];
  return connect(path, (events) => new TxikiHost(path, args, options.debug ?? false, events));
}
