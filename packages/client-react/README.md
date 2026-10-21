# gpui-bridge-react

A React renderer for [gpui-bridge](https://www.npmjs.com/package/gpui-bridge): native desktop
windows, drawn by GPUI and gpui-component, declared as React components.

```tsx
import { useState } from "react";
import { Button, Div, Text, render } from "gpui-bridge-react";

function Counter() {
  const [count, setCount] = useState(0);
  return (
    <Div class="flex flex-col items-center gap-4 p-6 size-full">
      <Text class="text-xl font-bold">{String(count)}</Text>
      <Button variant="primary" onClick={() => setCount(count + 1)}>Add</Button>
    </Div>
  );
}

await render(<Counter />, { window: { title: "Counter", width: 320, height: 200 } });
```

## Install

```sh
npm install gpui-bridge-react gpui-bridge react
```

`gpui-bridge` and `react` are **peer dependencies**: this package has no native binary of its
own. The host (`gpui-bridge-host`) travels inside `gpui-bridge`, for Windows x64 today.

Host lookup is the one from `gpui-bridge`: `createApp` options, `$GPUI_BRIDGE_HOST`, the bundled
asset, then a Cargo build in a checkout. `render` accepts the same host options.

## Links

- [gpui-bridge](https://www.npmjs.com/package/gpui-bridge), the underlying client.
- [React examples](https://github.com/InDIOS/gpui-bridge/tree/main/examples/react).
- [Source and issues](https://github.com/InDIOS/gpui-bridge).

Protocol version 1. MIT licensed.
