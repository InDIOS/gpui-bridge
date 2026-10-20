// File browser: a mid-size React app, written the way one would write it with Tailwind,
// to find what gpui-bridge is missing. Unknown classes show up as warnings with --debug.
// Run: pnpm dev react/files   (from the repository root)

import { readdirSync, statSync, openSync, readSync, closeSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { useMemo, useState } from "react";
import { Button, Checkbox, Div, Icon, Image, Input, render, Text } from "gpui-bridge-react";

interface Entry {
  name: string;
  dir: boolean;
  size: number;
}

const PREVIEW_BYTES = 4096;
const IMAGE_FILE = /.(png|jpe?g|gif|webp|svg|bmp|ico)$/i;

function list(path: string): Entry[] {
  const entries: Entry[] = [];
  for (const dirent of readdirSync(path, { withFileTypes: true })) {
    let size = 0;
    try {
      if (dirent.isFile()) size = statSync(join(path, dirent.name)).size;
    } catch {
      // Unreadable entries are still listed.
    }
    entries.push({ name: dirent.name, dir: dirent.isDirectory(), size });
  }
  return entries.sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name));
}

function preview(path: string): string {
  const fd = openSync(path, "r");
  try {
    const buffer = Buffer.alloc(PREVIEW_BYTES);
    const read = readSync(fd, buffer, 0, PREVIEW_BYTES, 0);
    const text = buffer.subarray(0, read).toString("utf8");
    // GPUI does not draw tabs.
    return text.includes("\0") ? "(binary file)" : text.replaceAll("\t", "    ");
  } finally {
    closeSync(fd);
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

function FilesApp({ start }: { start: string }) {
  const [path, setPath] = useState(start);
  const [draftPath, setDraftPath] = useState(start);
  const [filter, setFilter] = useState("");
  const [showHidden, setShowHidden] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const entries = useMemo(() => {
    try {
      return list(path);
    } catch (err) {
      return err as Error;
    }
  }, [path]);

  const visible =
    entries instanceof Error
      ? []
      : entries.filter(
          (entry) => (showHidden || !entry.name.startsWith(".")) && entry.name.toLowerCase().includes(filter.toLowerCase()),
        );

  const content = useMemo(() => {
    if (!selected || IMAGE_FILE.test(selected)) return null;
    try {
      return preview(join(path, selected));
    } catch (err) {
      return `Cannot read file: ${(err as Error).message}`;
    }
  }, [path, selected]);

  function go(target: string) {
    const next = resolve(target);
    try {
      if (!statSync(next).isDirectory()) throw new Error("Not a folder");
    } catch (err) {
      setError(`${next}: ${(err as Error).message}`);
      return;
    }
    setPath(next);
    setDraftPath(next);
    setSelected(null);
    setFilter("");
    setError(null);
  }

  return (
    <Div class="flex flex-col size-full bg-background text-foreground">
      <Div class="flex flex-row items-center gap-2 p-3 border-b border-border">
        <Button variant="ghost" disabled={dirname(path) === path} onClick={() => go(dirname(path))}>
          Up
        </Button>
        <Input class="flex-1" value={draftPath} onChange={setDraftPath} onSubmit={go} />
        <Button variant="secondary" onClick={() => go(homedir())}>
          Home
        </Button>
      </Div>

      {error && (
        <Div class="px-3 py-2 bg-danger">
          <Text class="text-sm text-danger-foreground">{error}</Text>
        </Div>
      )}

      <Div class="flex flex-row flex-1 min-h-0">
        <Div class="flex flex-col w-80 shrink-0 border-r border-border">
          <Div class="flex flex-col gap-2 p-3 border-b border-border">
            <Input placeholder="Filter" value={filter} onChange={setFilter} />
            <Checkbox label="Show hidden files" checked={showHidden} onChange={setShowHidden} />
          </Div>
          <Div class="flex flex-col flex-1 overflow-y-scroll">
            {entries instanceof Error && <Text class="p-3 text-sm text-danger">{entries.message}</Text>}
            {visible.map((entry) => (
              <Div
                key={entry.name}
                class={`flex flex-row items-center justify-between gap-2 px-3 py-1 cursor-pointer hover:bg-accent ${
                  entry.name === selected ? "bg-accent" : ""
                }`}
                onClick={() => (entry.dir ? go(join(path, entry.name)) : setSelected(entry.name))}
              >
                <Div class="flex flex-row flex-1 min-w-0 items-center gap-2">
                  <Icon
                    name={entry.dir ? "folder" : IMAGE_FILE.test(entry.name) ? "frame" : "file"}
                    class="size-4 shrink-0 text-muted-foreground"
                  />
                  <Text class={`flex-1 min-w-0 truncate text-sm ${entry.dir ? "font-medium" : ""}`}>{entry.name}</Text>
                </Div>
                {!entry.dir && <Text class="shrink-0 text-xs text-muted-foreground">{formatSize(entry.size)}</Text>}
              </Div>
            ))}
          </Div>
        </Div>

        <Div class="flex flex-col flex-1 min-w-0">
          {selected && IMAGE_FILE.test(selected) ? (
            <Div class="flex flex-1 min-h-0 p-3">
              <Image src={join(path, selected)} class="size-full" />
            </Div>
          ) : content === null ? (
            <Div class="flex flex-1 items-center justify-center">
              <Text class="text-sm text-muted-foreground">Select a file to preview it</Text>
            </Div>
          ) : (
            <Div class="flex flex-col flex-1 overflow-y-scroll p-3">
              <Text class="font-mono text-xs whitespace-pre">{content}</Text>
            </Div>
          )}
        </Div>
      </Div>

      <Div class="flex flex-row justify-between px-3 py-1 border-t border-border bg-muted">
        <Text class="text-xs text-muted-foreground">{path}</Text>
        <Text class="text-xs text-muted-foreground">
          {visible.length} {visible.length === 1 ? "item" : "items"}
        </Text>
      </Div>
    </Div>
  );
}

await render(<FilesApp start={resolve(process.argv.find((arg, i) => i > 1 && !arg.startsWith("--")) ?? ".")} />, {
  window: { title: "Files", width: 900, height: 600 },
  debug: process.argv.includes("--debug"),
});
