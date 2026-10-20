// React renderer for gpui-bridge: render a React tree into a native GPUI window.

import type { ReactNode } from "react";
import { ConcurrentRoot } from "react-reconciler/constants.js";
import { createApp, type App, type AppOptions, type WindowOptions } from "gpui-bridge";
import { reconciler } from "./host-config.js";

export * from "./components.js";

export interface RenderOptions extends AppOptions {
  window?: Omit<WindowOptions, "root">;
  /** Classes of the window root that holds the React tree. */
  rootClass?: string;
}

export interface Root {
  app: App;
  unmount(): void;
}

/** Starts the host and renders `element` into its window. */
export async function render(element: ReactNode, options: RenderOptions = {}): Promise<Root> {
  const app = await createApp(options);
  const root = app.div({ class: options.rootClass ?? "size-full" });
  // Sent with React's first commit.
  app.window({ ...options.window, root });

  const report = (error: unknown) => console.error(error);
  const fiberRoot = reconciler.createContainer(
    { app, root },
    ConcurrentRoot,
    null,
    false,
    null,
    "",
    report,
    report,
    report,
    () => {},
    null,
  );
  reconciler.updateContainer(element, fiberRoot, null, null);
  return { app, unmount: () => reconciler.updateContainer(null, fiberRoot, null, null) };
}
