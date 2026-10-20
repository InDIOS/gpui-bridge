// Spawns the host binary and speaks JSON Lines over its stdin/stdout.

import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import type { HostEvents, HostExit, HostTransport } from "./app.js";
import type { ClientMessage, HostMessage } from "./protocol.js";

const STDERR_TAIL_LINES = 20;

export interface HostProcessOptions extends HostEvents {
  path: string;
  args: string[];
  debug: boolean;
}

export class HostProcess implements HostTransport {
  private readonly child: ChildProcess;
  private readonly stderrTail: string[] = [];
  private exited = false;

  constructor(options: HostProcessOptions) {
    this.child = spawn(options.path, options.args, { stdio: ["pipe", "pipe", "pipe"] });

    createInterface({ input: this.child.stdout! }).on("line", (line) => {
      let message: HostMessage;
      try {
        message = JSON.parse(line);
      } catch {
        options.onCorruptLine(line);
        return;
      }
      options.onMessage(message);
    });

    createInterface({ input: this.child.stderr! }).on("line", (line) => {
      this.stderrTail.push(line);
      if (this.stderrTail.length > STDERR_TAIL_LINES) this.stderrTail.shift();
      if (options.debug) console.error(`[host] ${line}`);
    });

    // Writes racing a dying host fail with EPIPE; the exit handler reports the real cause.
    this.child.stdin!.on("error", () => {});

    const finish = (exit: Omit<HostExit, "stderrTail">) => {
      if (this.exited) return;
      this.exited = true;
      options.onExit({ ...exit, stderrTail: [...this.stderrTail] });
    };
    this.child.on("error", (spawnError) => finish({ code: null, signal: null, spawnError }));
    // "close" (not "exit") so all stdout/stderr lines are delivered first.
    this.child.on("close", (code, signal) => finish({ code, signal }));
  }

  get running(): boolean {
    return !this.exited;
  }

  send(message: ClientMessage): void {
    if (this.exited) return;
    this.child.stdin!.write(JSON.stringify(message) + "\n");
  }

  /** Closes stdin; the host exits on EOF. */
  close(): void {
    this.child.stdin!.end();
  }

  kill(): void {
    this.child.kill();
  }
}
