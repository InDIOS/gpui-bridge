# gpui-bridge — Historial

Qué hizo cada fase del desarrollo, con sus criterios de aceptación y cómo se verificó. Es un
registro: lo escrito no se reescribe, y cada fase nueva se agrega al final de la sección
"Fases". El plan vigente está en [GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md).

Las referencias "la sección N" de las primeras fases son del plan de ese momento. "§8" y "§9"
en las fases 17 a 27 son las dos tandas de pasos del final de este documento (antes estaban en
el plan). "PROTOCOL.md §N" apunta a las secciones de [PROTOCOL.md](PROTOCOL.md). Las rutas y
nombres de archivo son los de cada momento (por ejemplo, `host/src/render.rs` se dividió
después de la fase 27).

> **Nota (reorganización posterior al monorepo).** Las rutas de este documento son históricas.
> Hoy el host y los clientes viven bajo `packages/`, los documentos bajo `docs/`, `bench/` pasó a
> `benchmarks/` y los ejemplos React a `examples/react/`. Los enlaces se actualizaron; el texto no
> se reescribe. La estructura vigente está en `docs/GPUI_BRIDGE_PLAN.md` §4 y en
> `docs/adr/0001-monorepo.md`.

## Fases

### Fase 0 — Configuración

1. Crear la estructura de carpetas de la sección 4.
2. Inicializar `host/` con `cargo init --bin`.
3. Agregar dependencias: `gpui` y `gpui-component` (versiones compatibles, ver regla 7), `serde`, `serde_json`, y lo necesario para logging a stderr.
4. Inicializar `client-node/` con TypeScript.

**Criterios de aceptación:** `cargo build` compila en `host/`; `npm run build` compila en `client-node/`.

**Estado:** completada. Se usa `gpui-kit = "0.7"`, como indica el README de gpui-component 0.7.3: fija la versión de GPUI compatible (`gpui-pre =0.3.9`) y reexporta GPUI (`gpui_kit::*`) y gpui-component (`gpui_kit::component`). Logging: `log` + `env_logger` hacia stderr.

### Fase 1 — Host mínimo

1. En `main.rs`, abrir una ventana GPUI vacía siguiendo los ejemplos actuales de gpui-component (incluida su inicialización de tema).
2. En `transport.rs`, leer stdin línea por línea en un **hilo separado** (GPUI necesita el hilo principal) y enviar los mensajes a la app mediante un canal. Investiga en los ejemplos y en el código de GPUI la forma idiomática de recibir datos de otro hilo dentro de la app (por ejemplo, una tarea en primer plano que espere el canal).
3. Escribir a stdout solo desde `transport.rs`, con flush después de cada mensaje.
4. Agregar un flag `--no-window` que procese mensajes sin abrir ventana, para pruebas automatizadas.

**Criterios de aceptación:** `echo '{"op":"hello","protocol":1}' | cargo run -- --no-window` imprime `{"type":"ready","protocol":1}` en stdout. Con ventana, la app abre y cierra limpiamente.

**Estado:** completada. El canal stdin → app es `futures::channel::mpsc::unbounded`: la app lo consume con `cx.spawn` y `--no-window` con `block_on_stream`. Si stdin llega a EOF (el cliente se fue), el host termina. Al cerrar la última ventana se llama `cx.quit()`. En Windows, GPUI escribe en stderr `0x887A002D` (falta la capa de depuración de DirectX) y errores `Invalid window handle` al cerrar la ventana; son internos de GPUI y aparecen también sin nuestro código.

### Fase 2 — Protocolo

1. Definir en `protocol.rs` los enums de mensajes con `serde` (`#[serde(tag = "op")]` para entrada y `#[serde(tag = "type")]` para salida).
2. Implementar el handshake de la sección 2 de PROTOCOL.md.
3. Manejar errores según la regla 4 y la lista de códigos de PROTOCOL.md §6.

**Criterios de aceptación:** tests unitarios de serialización/deserialización para cada tipo de mensaje; un JSON inválido produce `invalid_json` y el host sigue procesando las líneas siguientes.

**Estado:** completada. `protocol::parse_client_message` clasifica los errores (ver PROTOCOL.md §6). En `window`, todos los campos son opcionales para poder actualizar solo una parte. Hay tests unitarios en `protocol.rs` y tests de integración del binario en `host/tests/` (las transcripciones de `conformance/` y `no_window.rs`).

### Fase 3 — Árbol retenido

1. En `tree.rs`, implementar un mapa de ID a nodo (tipo, props, eventos suscritos, lista ordenada de hijos, padre).
2. Implementar `create`, `append`, `insertBefore`, `setProps` (con merge y `null`), `setEvents`, `remove` (recursivo).
3. Acumular operaciones pendientes y aplicarlas en `commit`.
4. En modo `--no-window`, después de cada commit, escribir en stderr una representación legible del árbol.

**Criterios de aceptación:** tests unitarios para cada operación, incluidos casos de error (padre inexistente, ID duplicado, remove de un nodo con hijos).

**Estado:** completada. `tree::Tree` guarda los nodos, la ventana (`WindowSpec`, que también se aplica en `commit`) y la cola de operaciones pendientes. `Session` (en `main.rs`) maneja el handshake y el árbol. Aún no se validan los tipos de las props (`invalid_props`); eso llega en las fases 4–6, cuando el render los use.

### Fase 4 — Render de `div` y `text`

1. En `render.rs`, crear la vista raíz que recorra el árbol desde el nodo `root` y construya elementos GPUI.
2. Al aplicar un commit, notificar a GPUI que la vista cambió para que vuelva a renderizar.
3. Por ahora, aplicar solo `flex`, `flex-col`, `gap-{n}` y `p-{n}` de forma directa.

**Criterios de aceptación:** enviando mensajes a mano por stdin se ve en pantalla un `div` con dos textos dispuestos en columna.

**Estado:** completada. La ventana ya no se abre al arrancar: se abre en el primer `commit` que define `root` mediante `window` (por defecto 800×600). Después, solo se aplican el título o el tamaño si cambiaron. `Session` es una entidad de GPUI y notifica después de cada `commit`; `RootView` la observa y vuelve a renderizar. Escala de espaciado verificada en `gpui-pre-macros`: el sufijo `N` equivale a `rems(N × 0.25)`, igual que en Tailwind. Hasta la fase 6, `Button`, `Input` y `Checkbox` se renderizan como `div`.

**DPI (verificado):** `width`/`height` son píxeles lógicos. GPUI en Windows es consciente del DPI de cada monitor: convierte el tamaño inicial con la escala del monitor y atiende `WM_DPICHANGED`. Probado moviendo la ventana entre un monitor al 100 % y otro al 150 %: el tamaño físico pasa de 456×439 a 684×659 (×1,5), el contenido escala nítido y con el mismo layout, la interacción (clics, escritura) funciona y al volver al 100 % todo queda igual, conservando el estado. No se probó 200 %, que sigue la misma ruta de código.

### Fase 5 — Traducción de clases

1. En `styles.rs`, implementar el parser de clases de la sección 8 de PROTOCOL.md con caché por string.
2. Verificar en el código de GPUI los métodos de estilo disponibles y su escala de espaciado antes de mapear.
3. Para los colores, leer los valores del tema activo de gpui-component.

**Criterios de aceptación:** tests unitarios del parser (clases válidas, desconocidas, repetidas); un ejemplo visual con todas las clases del MVP.

**Estado:** completada. La caché guarda la lista de clases interpretada (`Rc<[Class]>`) por string; los colores se guardan como roles del tema y se resuelven en cada render desde `cx.theme().colors` (gpui-component), así que un cambio de tema no invalida la caché. Las clases se aplican en orden: con `p-2 p-4` gana `p-4`. Valores verificados en GPUI: `border` = 1px; `rounded-sm|md|lg` = 0.25|0.375|0.5rem; `rounded-full` = 9999px; `w-{n}`/`h-{n}` usan la escala de espaciado. Se agregó `text-primary-foreground`: sin esa clase no había forma de escribir texto legible sobre `bg-primary`. El tema oscuro (`window.theme`) quedó verificado visualmente, incluido el cambio en caliente claro → oscuro → claro.

### Fase 6 — Componentes de gpui-component

1. Implementar adaptadores en `components/` para `Button`, `Input` y `Checkbox`, mapeando las props de la sección 5 de PROTOCOL.md a la API real de cada componente.
2. `Input` mantiene su propio estado en el host (ver PROTOCOL.md §7).

**Criterios de aceptación:** los tres componentes se ven y responden a la interacción en la ventana, y `setProps` actualiza su estado visible.

**Estado:** completada.
- `components/` crea `Button`, `Checkbox` e `Input` de gpui-component; `class` se aplica sobre el propio componente.
- `Input`: un `InputState` por nodo, que vive en `RootView`. Se sincroniza después de cada `commit` mediante `observe_in` (necesita `window`). El texto se reemplaza cada vez que el cliente envía `value` en `create`/`setProps`, aunque el valor sea igual al anterior (`Tree::take_value_writes`).
- `Checkbox`: el estado `checked` también vive en el host; un clic lo cambia al instante (`Tree::set_host_prop`) y un `setProps` posterior lo sobrescribe.
- Validación de props según PROTOCOL.md §5: un tipo o valor incorrecto rechaza la operación con `invalid_props`; una prop desconocida solo avisa una vez en stderr.
- Se registran los assets de iconos (`with_assets(gpui_kit::assets::Assets)`); sin ellos, el checkbox no dibuja la marca.
- Verificado con clics y teclado reales: escribir, marcar, `setProps` (etiqueta, variante, habilitar, limpiar el input dos veces seguidas, desmarcar), en tema claro y oscuro.
- Modos sincronizado y controlado (PROTOCOL.md §5) para `Input` y `Checkbox`. Los eventos `change` de estos dos componentes ya se emiten (se adelantaron de la fase 7 porque la sincronización los necesita), y se verificaron en stdout junto con aceptar, transformar y rechazar ediciones en modo controlado.

### Fase 7 — Eventos

1. Conectar los handlers de GPUI para emitir los eventos de la sección 6 de PROTOCOL.md, solo si el nodo los tiene suscritos.
2. Emitir `windowClosed` cuando se cierre la ventana y terminar el proceso del host.

**Criterios de aceptación:** al hacer click en un botón suscrito, aparece en stdout `{"type":"event","id":...,"event":"click"}`; un botón no suscrito no emite nada.

**Estado:** completada.
- `click` en `Button` y `div`: el handler solo se instala si el nodo está suscrito; un `div` suscrito recibe un id de elemento para ser interactivo.
- `submit` de `Input` cuando se pulsa Enter (`InputEvent::PressEnter`), con el texto actual.
- `change` se había adelantado en la fase 6.
- `windowClosed` se emite cuando el usuario cierra la ventana, y después el host termina. Si el cliente cierra stdin, el host termina sin emitir `windowClosed`.
- Verificado con clics y teclado reales: botón y `div` suscritos emiten; no suscrito, deshabilitado y `div` sin suscripción no emiten; `setEvents` suscribe y desuscribe en caliente; `submit` lleva el valor; `windowClosed` y salida con código 0.

### Fase 8 — Cliente Node

1. `host-process.ts`: lanzar el binario del host con `child_process.spawn`, leer stdout por líneas, reenviar stderr a la consola en modo debug, manejar la salida del proceso.
2. `protocol.ts`: tipos TypeScript equivalentes a `protocol.rs`.
3. `tree.ts`: asignación incremental de IDs y acumulación de operaciones; `commit()` envía todo y luego el mensaje `commit`.
4. `index.ts`: API pública mínima, por ejemplo:

```ts
const app = await createApp({ hostPath, debug: true });
const root = app.div({ class: "flex flex-col gap-2 p-4" });
const label = app.text({ content: "Clicks: 0" });
const button = app.button({ label: "Sumar", variant: "primary" });
root.append(label, button);
button.on("click", () => { /* ... */ });
app.window({ title: "Contador", width: 400, height: 300, root });
app.commit();
```

La forma exacta de la API puede ajustarse, pero debe seguir siendo así de directa.

**Criterios de aceptación:** el cliente completa el handshake, crea un árbol y recibe eventos tipados; si el host se cae, el cliente lo reporta con un error claro.

**Estado:** completada.
- API: `createApp({ hostPath?, debug?, headless? })` → `App` con `div/text/button/input/checkbox`, `window`, `commit`, `on("error" | "windowClosed" | "exit")`, `close()` y `exited()`. Los nodos (`UINode<T>`) tienen `append`, `insertBefore`, `set` (`null` quita una prop), `on`, `off` y `remove`. Las props y los payloads de eventos están tipados por tipo de nodo (`NodeProps`, `NodeEvents` en `protocol.ts`).
- `node.on(evento)` suscribe el evento en el host de forma automática (`setEvents` cuando cambia el conjunto de suscripciones).
- `hostPath` por defecto: `$GPUI_BRIDGE_HOST` o `host/target/debug/gpui-bridge-host[.exe]` del repositorio. `headless` lanza el host con `--no-window`.
- Errores: si el binario no existe, `createApp` lo rechaza indicando la ruta y cómo compilar; si el host termina de forma inesperada, se emite `exit` con el código y las últimas líneas de stderr (si nadie escucha, se imprime con `console.error`) y `commit()` lanza `HostExitedError`. Un `error` del host sin listener se muestra con `console.warn`.
- TypeScript 7 no carga `@types/*` automáticamente: `tsconfig.json` declara `"types": ["node"]`.
- Tests: `npm test` (6 tests con `node:test` contra el host real en modo headless). Verificado también de punta a punta con ventana: clics y Enter llegan como eventos tipados, el cliente actualiza la UI y al cerrar la ventana recibe `windowClosed` y termina.

### Fase 9 — Ejemplos

1. `examples/counter.ts`: un texto y dos botones (sumar y restar).
2. `examples/todo.ts`: un `Input` para agregar tareas con `submit`, una lista de `Checkbox` y un botón para borrar las completadas.

**Criterios de aceptación:** ambos ejemplos funcionan de punta a punta sin tocar código Rust.

**Estado:** completada.
- Ejecutar: `cargo build` en `host/`, `npm run build` en `client-node/`, y luego `node examples/counter.ts` o `node examples/todo.ts` (Node 24 ejecuta TypeScript directamente; `--debug` muestra los logs del host). `examples/package.json` declara `"type": "module"` (necesario para el `await` de nivel superior).
- Comprobar tipos: `client-node/node_modules/.bin/tsc -p examples`.
- `todo.ts` usa checkboxes sincronizados: el host los marca y el evento `change` mantiene la copia del cliente, que actualiza el pie ("N items left") y habilita o deshabilita "Clear completed".
- Verificado con clics y teclado reales: el contador (3 × Add, 1 × Subtract → 2); la lista de tareas (añadir tres, ignorar un envío vacío, marcar dos, limpiar las completadas, añadir otra después). Al cerrar la ventana, Node termina con código 0.

### Fase 10 (opcional) — Renderer de React

Solo cuando las fases 0 a 9 estén completas.

1. Crear un paquete aparte que use `react-reconciler` sobre la API de la fase 8.
2. Mapear la configuración del reconciler (`createInstance`, `appendChild`, `insertBefore`, `commitUpdate`, `removeChild`, `resetAfterCommit`) a las operaciones del protocolo; `resetAfterCommit` envía el `commit`.
3. Reescribir `todo.ts` como componente de React con hooks.

**Criterios de aceptación:** el ejemplo de tareas en React se comporta igual que la versión de la fase 9.

**Estado:** completada. Cambio respecto del plan: `examples/todo.ts` no se reescribió; se agregó un ejemplo nuevo en React.
- Paquete `client-react/` (`gpui-bridge-react`): `react` 19.3.0 (peer), `react-reconciler` 0.34.0 y `gpui-bridge` enlazado (`link:../client-node`).
- API: `render(<App />, { window, rootClass, ...AppOptions })` → `{ app, unmount }`. Componentes `Div`, `Text`, `Button`, `Input` y `Checkbox` (constantes con el tipo de nodo, tipadas como componentes para JSX). Props de React: `onClick`, `onChange(valor)`, `onSubmit(valor)`; el texto hijo de `Text`/`Button` se convierte en `content`/`label`.
- Igual que en React DOM, un `Input` con `value` o un `Checkbox` con `checked` es controlado (`controlled: true`): el estado de React manda.
- Host config: `createInstance`/`createTextInstance` crean nodos, `appendChild`/`insertBefore`/`removeChild` (y sus variantes de contenedor) los mueven o borran, `commitUpdate` envía solo las props que cambian (`null` para las quitadas) y ajusta las suscripciones, y `resetAfterCommit` envía `commit` (un render del host por cada commit de React).
- El ejemplo está en `client-react/examples/todo.tsx` y no en `examples/`, para compartir la misma instancia de React que el renderer (dos copias de React rompen los hooks). Ejecutar: `npm run example` en `client-react/`.
- Tests: `npm test` en `client-react/` (3 tests headless que comprueban las operaciones del protocolo que genera cada cambio de estado). Verificado de punta a punta con clics y teclado: el mismo recorrido que `todo.ts` da el mismo resultado.

### Extra — Cliente para txiki.js y tamaño de distribución

- El cliente se separó en un núcleo independiente del runtime (`app.ts`: `App`, `UINode`, `connect(hostPath, start)`) y transportes: `host-process.ts` (Node, `child_process`) y `txiki.ts` (txiki.js, `tjs.spawn` + Web Streams). La API es la misma; `createApp` (Node) y `createTxikiApp` (txiki.js).
- `createTxikiApp` busca el host en `$GPUI_BRIDGE_HOST` o, si no, al lado del ejecutable (así se distribuye una app compilada).
- Ejecutar: `tjs run examples/counter-txiki.js` (con `GPUI_BRIDGE_HOST` apuntando al host). Compilar un ejecutable autónomo: `tjs compile counter-txiki.js <salida>.exe` **ejecutado desde `examples/`** (resuelve los `import` relativos al directorio actual); incluye los módulos importados.
- Probado con txiki.js v26.6.0 en Windows, con clics reales, tanto con `tjs run` como con el ejecutable compilado en una carpeta aislada junto al host.
- Tamaños medidos en Windows (MB, en disco / zip):

| Pieza | En disco | Zip |
|---|---|---|
| Host, release optimizada (LTO, strip, `panic=abort`) | 14,0 | 6,2 |
| Host, release normal | 23,2 | — |
| App txiki.js compilada (runtime + JS) | 5,3 | 2,7 |
| Node 24 (`node.exe`) | 89,5 | 34,5 |

  Total de una app: unos 19 MB en disco / 9 MB en zip con txiki.js, contra unos 105 MB / 41 MB con Node incluido.

### Fase 11 — Cliente de Python

Primer cliente en un lenguaje distinto de JavaScript, para comprobar que el protocolo es realmente neutral.

- `client-python/gpui_bridge.py`: requiere Python ≥ 3.10 (`match`, `int | None`). Un solo módulo, solo biblioteca estándar (`subprocess`, `threading`, `queue`, `json`). Misma API que el cliente de Node: `App(host_path=None, debug=False, headless=False)` hace el handshake al construirse; `app.div/text/button/input/checkbox(**props)`, `node.append/insert_before/set/on/off/remove`, `app.window(...)`, `app.commit()`.
- `class` es palabra reservada en Python: se escribe `class_` (se quita el `_` final de cada prop).
- Los mensajes del host los lee un hilo y se encolan; `app.run()` bloquea y llama a los handlers en el hilo que lo invoca, hasta que el host termina (ventana cerrada, `close()` o crash). Así los handlers pueden llamar a `commit()` sin locks. Los handlers reciben el texto (`Input`), el estado (`Checkbox`) o nada (`click`).
- Errores del host: `app.on_error = callback` (sin callback se imprimen en stderr). `app.on_window_closed` para el cierre.
- Ejecutar: `python examples/counter.py` (después de `cargo build` en `host/`; `--debug` muestra los logs del host). Tests: `python -m unittest` en `client-python/` (6 tests headless, los mismos casos que los de Node).
- Probado con Python 3.15 (instalado con mise) en Windows, con clics reales: tres en Add y uno en Subtract dan 2, y al cerrar la ventana terminan el cliente y el host sin errores.
- No hizo falta tocar el host ni el protocolo.

### Extra — PROTOCOL.md y transcripciones de conformidad

- La especificación salió de la §5 de este plan a [PROTOCOL.md](PROTOCOL.md), en inglés y con las mismas secciones (1–8), más la §9, que describe el formato de las transcripciones.
- `conformance/*.txt`: una sesión por archivo contra `host --no-window`. Las líneas `>` se envían tal cual y las `<` son las respuestas esperadas, en orden; solo se comparan los campos escritos. Cubren el handshake, mensajes antes de `hello`, errores de parseo, errores del árbol (ciclos, mover nodos, `remove` en cascada), validación de props y la ventana.
- `host/tests/conformance.rs` las ejecuta con `cargo test`. Reemplazaron tres de los cuatro tests de `no_window.rs`; queda solo el que comprueba que el host termina solo tras `protocol_mismatch` con stdin abierto, porque una transcripción cierra stdin. Se verificó que el runner falla con un código equivocado y con una respuesta no declarada.
- Los eventos y `windowClosed` necesitan una ventana real y no están cubiertos.

### Fase 12 — Temas personalizados

Hasta aquí solo había claro y oscuro con el tema por defecto de gpui-component, así que toda app se veía igual.

1. `window` acepta `themeSet`: un archivo de tema de gpui-component (colores, `radius`, fuentes, `shadow`), en lugar del tema por defecto (PROTOCOL.md §3).
2. Las clases de color aceptan los colores principales del tema: `bg-`, `text-` y `border-` con `secondary`, `accent`, `danger`, `success`, `warning`, `info`, `popover`, `input`, `ring` y sus `-foreground` (PROTOCOL.md §8).

**Criterios de aceptación:** un `themeSet` cambia los colores, el radio y las fuentes de los componentes y de las clases, en claro y oscuro y en caliente; `{"themes":[]}` vuelve al tema por defecto; un `themeSet` mal tipado da `invalid_json`.

**Estado:** completada.
- `protocol::CustomThemes` deserializa el `ThemeSet` de gpui-component y lo guarda en un `Arc`; se compara por identidad, así que cada mensaje con `themeSet` lo vuelve a aplicar. Un campo mal tipado o un `mode` desconocido rechaza el mensaje con `invalid_json` (lo hace serde).
- `render::sync_window` toma el primer tema de cada modo (si falta, usa el tema por defecto) y llama a `Theme::change`, que lo aplica y repinta. Antes resetea fuentes, radios y `shadow` a los valores de gpui-component, porque `apply_config` solo los escribe cuando el archivo los define; sin eso, el `radius` de un tema seguía activo al volver al tema por defecto (se vio en la prueba).
- Los colores que no se pueden parsear los ignora gpui-component sin avisar, y usa su valor de fábrica. Ese valor puede diferir un poco del tema "Default" (por ejemplo, `success` en claro).
- Clientes: `themeSet` en `WindowOptions` y los tipos `ThemeSet`/`ThemeConfig` en el cliente de Node; React lo hereda por `render(..., { window })`. En Python funciona sin cambios: `app.window(themeSet={...})`.
- Ejemplo: `examples/theme.ts` alterna entre el tema por defecto y uno propio, en claro y oscuro. Verificado con clics reales: colores, radio y componentes (`Button`, `Input`, `Checkbox`) siguen el tema, y al volver al tema por defecto el radio vuelve a 6.
- Tests: casos nuevos en `protocol.rs` y `styles.rs`, y en `conformance/window.txt` (un `themeSet` válido y dos mal tipados).

### Fase 13 — App real: explorador de archivos

Una app mediana para descubrir qué le falta a gpui-bridge, en vez de adivinarlo. Las clases se escribieron como en Tailwind, sin mirar cuáles soporta el host; las que no conoce aparecen como avisos con `--debug`.

- `client-react/examples/files.tsx`: barra de ruta (`Input` con Enter, botones Up y Home), filtro, checkbox de archivos ocultos, lista de carpetas y archivos con tamaño, vista previa de texto (primeros 4 KB) y barra de estado. Ejecutar: `npm run files -- <carpeta>` en `client-react/`.
- Verificado con clics y teclado reales: entrar a carpetas, Up, Home, filtrar escribiendo, mostrar ocultos y seleccionar un archivo (resaltado con `bg-accent`). El protocolo no dio ningún error y la app nunca necesitó tocar el host.

**Hallazgos**, de más a menos grave (entrada para la siguiente fase):

1. **No hay scroll.** Una lista larga (la carpeta de usuario con ocultos) o una vista previa larga se salen de la ventana: no se ven y empujan la barra de estado fuera. Faltan `overflow-y-scroll` / `overflow-hidden` y `min-h-0` / `min-w-0` para que un hijo `flex-1` pueda achicarse.
2. **No hay bordes de un solo lado:** `border-t|r|b|l`. Sin ellos no se pueden separar los paneles.
3. **Los anchos fijos se encogen:** la columna `w-80` se achica cuando la vista previa tiene texto largo. Faltan `shrink-0` / `flex-none`.
4. **Texto:** faltan `text-xs`, `truncate` (nombres largos), `font-mono` y `whitespace-pre` (código y archivos de texto).
5. **Interacción:** las filas clicables no muestran nada al pasar el mouse. Faltan `hover:` (al menos `hover:bg-*`) y `cursor-pointer`.
6. **Componentes que se extrañan:** iconos (gpui-component ya trae `Icon` y los SVG están embebidos), y para listas grandes una lista virtualizada. La lista de la carpeta de usuario (unas 100 entradas) respondió bien.
7. **Eventos que se extrañan:** doble clic (abrir con doble clic y seleccionar con uno) y teclado en la lista (flechas, Enter).

### Fase 14 — Scroll, clases de la app real e iconos

Resuelve los hallazgos 1–5 de la fase 13 y agrega iconos. Quedan para después la lista virtualizada y los eventos nuevos (doble clic, teclado), que cambian el protocolo de eventos.

**Criterios de aceptación:** el explorador de archivos se ve y funciona sin avisos de clases desconocidas: listas y vista previa con scroll, paneles separados por bordes, columna de ancho fijo, nombres largos truncados, vista previa monoespaciada, hover en las filas e iconos de carpeta y archivo.

**Estado:** completada.
- Clases nuevas (PROTOCOL.md §8): `overflow-hidden`, `overflow-scroll|x-scroll|y-scroll`, `min-w-0`, `min-h-0`, `shrink-0`, `flex-none`, `size-{n}`, `border-t|r|b|l`, `text-xs`, `font-mono`, `truncate`, `whitespace-normal|nowrap|pre`, `cursor-pointer` y `hover:` con un color del tema.
- Scroll: un `div` con `overflow-*-scroll` se envuelve con `overflow_y_scrollbar()` (y sus variantes) de gpui-component, con id `("scroll", id)`, así cada nodo guarda su posición y muestra la barra del tema. Solo en `div`; en otros nodos la clase no hace nada.
- `hover:` solo acepta colores del tema (`hover:bg-*`, `hover:text-*`, `hover:border-*`) y solo en `div`: es lo que pide una fila clicable, y `.hover()` de GPUI necesita un elemento interactivo.
- `font-mono` usa la fuente monoespaciada del tema, así que `styles::apply` ahora recibe el `Theme` completo y no solo sus colores.
- `whitespace-pre` es igual a `nowrap`: GPUI ya conserva espacios y saltos de línea. Los tabuladores no se dibujan (limitación de GPUI, documentada); el ejemplo los reemplaza por espacios.
- Nodo `Icon` con `name`: uno de los 101 iconos que gpui-component ya embebe (`gpui_kit::assets::Assets`), así que el binario no crece. Un nombre desconocido da `invalid_props`. Si hiciera falta el catálogo completo de Lucide (1.830 iconos), se registra `AllAssets`, que según gpui-kit suma cerca de 1 MB.
- Clientes: `app.icon()` en Node (y txiki.js) y Python, y el componente `Icon` en React.
- Verificado con clics, rueda del mouse y teclado reales en `files.tsx`: scroll de la lista (96 entradas) y de la vista previa con barra visible, hover en las filas, nombre largo truncado con "…", vista previa monoespaciada con la sangría conservada, columna izquierda de ancho fijo y barra de estado siempre visible. Sin avisos de clases desconocidas.
- Tests: casos nuevos en `styles.rs`, `conformance/props.txt` (`Icon` válido, desconocido, con `../` y mal tipado) y un test de React para `Icon`.

### Fase 15 — Tests automáticos de eventos

Hasta aquí los eventos y `windowClosed` solo se probaban a mano en una ventana, porque las transcripciones corren con `--no-window`.

**Criterios de aceptación:** las transcripciones cubren `click`, `change`, `submit` y `windowClosed`, con la misma lógica que usa la ventana; la ventana real sigue funcionando igual.

**Estado:** completada.
- Mensaje `simulate`, solo con `--no-window` (PROTOCOL.md §9): reproduce una acción del usuario sobre el árbol ya confirmado (`commit`). Sin `--no-window` es `unknown_op`, así que no forma parte del protocolo de los clientes. Se maneja en `simulate.rs`, antes del parseo normal, y no es una variante de `ClientMessage`.
- La lógica de las acciones salió de los closures de GPUI a funciones compartidas en `components/mod.rs` (`clicked`, `checkbox_clicked`, `input_edited`, `input_submitted`). Los handlers de la ventana y `simulate` llaman a las mismas, así que las transcripciones prueban el código real y no una copia. La única regla repetida es la de `disabled`, que en la ventana aplica GPUI.
- `conformance/events.txt`: clic suscrito y no suscrito, botón deshabilitado, checkbox sincronizado (dos clics dan `true` y `false`), checkbox controlado (dos clics dan `true` y `true` hasta el `setProps`), `change` y `submit` del Input con su texto, un `setEvents`/`create` pendiente que no cuenta hasta el `commit`, errores (`unknown_node`, `invalid_operation`, `invalid_json`) y `windowClosed` con salida limpia. Se comprobó que el runner falla al cambiar una respuesta esperada.
- Verificado de nuevo en una ventana real con `examples/todo.ts`: escribir y Enter agrega tareas, el checkbox marca y actualiza el pie, y cerrar con la X termina el host y el cliente con código 0.
- No cubierto: que un clic o un Enter reales lleguen al handler (son una línea de GPUI cada uno) y la reversión del Input controlado, que necesita el `InputState` de la ventana. Siguen probándose a mano.

### Fase 16 — Empaquetado local

Hasta aquí los clientes buscaban el host en `$GPUI_BRIDGE_HOST` o en `host/target/debug` del repositorio, así que no funcionaban instalados en otro proyecto.

**Criterios de aceptación:** los paquetes de Node, React y Python se instalan en una carpeta fuera del repositorio y abren su ventana sin `GPUI_BRIDGE_HOST` ni rutas a mano; dentro del repositorio todo sigue usando la build de debug.

**Estado:** completada.
- `host/Cargo.toml` tiene un perfil `release` para distribuir: LTO, `codegen-units = 1`, `strip` y `panic = "abort"`. El binario mide 15,2 MB (antes 14,0; el host creció en las fases 12–15) y compilarlo tarda unos 5 minutos.
- Orden de búsqueda del host, igual en Node y Python: la opción explícita (`hostPath` / `host_path`), `$GPUI_BRIDGE_HOST`, el host incluido en el paquete (`bin/`) y, si no hay, la build de debug del repositorio. En un checkout no hay `bin/`: se crea solo al empaquetar y se borra al terminar, para no usar un host viejo mientras se desarrolla.
- Node: `pnpm pack` en `client-node/` corre `prepack` (`tsc` y `scripts/bundle-host.js`: `cargo build --release` y copia a `bin/`) y `postpack` (borra `bin/`). `files` publica `dist` y `bin`. Tarball: 6,7 MB.
- React: `gpui-bridge` pasó a `peerDependencies` (como `react`), con `link:../client-node` solo en `devDependencies`, porque `link:` no funciona fuera del repositorio. `files` publica solo `dist/src`. Se instala junto al tarball de `gpui-bridge`.
- Python: el módulo pasó a ser el paquete `gpui_bridge/` (`__init__.py`), con `pyproject.toml` (setuptools, que pip descarga solo para construir). `python build_wheel.py` compila el host, lo copia a `gpui_bridge/bin/`, arma el wheel en `dist/` y limpia. Wheel: 6,7 MB. Atajo: el wheel queda marcado `py3-none-any` aunque lleva un binario de Windows; antes de compartirlo necesita la etiqueta de plataforma.
- txiki.js ya buscaba el host junto a su ejecutable (sin cambios).
- Verificado fuera del repositorio, en una carpeta vacía y sin `GPUI_BRIDGE_HOST`: `npm install` de los dos tarballs más `react`, y una app React con `Icon` y un botón (dos clics dan 2); un `venv` nuevo con el wheel y una app de Python (tres clics dan 3, cerrar con la X sale con código 0). En los dos casos el host que corre es el que viene en el paquete. Dentro del repositorio los tests siguen usando `host/target/debug`.

### Fase 17 — Imágenes

Primer paso de la §8.

**Criterios de aceptación:** un nodo `Image` muestra archivos locales en los formatos comunes, con los modos de `object-fit`; un archivo que cambia en disco se puede volver a mostrar; un archivo que falta no rompe nada.

**Estado:** completada.
- Nodo `Image` con `src` (ruta de archivo) y `fit` (`contain` por defecto, `cover`, `fill`, `scale-down`, `none`), dibujado con `img()` de GPUI. La ruta se pasa como `PathBuf`: como texto, GPUI toma `C:...` por una URL con esquema `c`.
- Solo archivos locales: la app de GPUI trae un cliente HTTP vacío, así que las URLs no cargan. Agregar un cliente HTTP queda para cuando haga falta.
- GPUI guarda cada imagen por ruta y no vuelve a leer el archivo aunque cambie (se comprobó). Por eso, cada vez que el cliente envía `src`, aunque sea la misma ruta, el host la saca del caché (`ImageSource::remove_asset`) después del `commit`; funciona como `value` en `Input` (`Tree::take_image_reloads`). Verificado: reemplazar el archivo y reenviar el mismo `src` muestra la imagen nueva.
- Un archivo que falta deja el nodo vacío. El error de GPUI no dice qué archivo era, así que el host avisa en stderr con la ruta cuando recibe un `src` que no existe (no es un error: el archivo puede aparecer después).
- Los SVG se rasterizan a su tamaño propio y se ven borrosos si se agrandan mucho (documentado; para eso está `Icon`).
- Clientes: `app.image()` en Node (y txiki.js) y Python, y `Image` en React.
- `client-react/examples/files.tsx` ahora muestra la vista previa de imágenes. Verificado en ventana real con PNG, JPG y SVG, los cinco modos de `fit` lado a lado y un archivo inexistente.
- Tests: casos en `conformance/props.txt` (válido, `fit` inválido, `src` mal tipado, quitar `fit` con `null`) y un test unitario de la recarga.

### Fase 18 — Ventanas sin marco

Segundo paso de la §8.

**Criterios de aceptación:** una ventana sin la barra de título del sistema se puede arrastrar, maximizar, restaurar, minimizar y cerrar desde una barra dibujada por la app, y los controles normales dentro de esa barra siguen funcionando.

**Estado:** completada.
- `window` acepta `frameless: true`: abre la ventana con `TitlebarOptions { appears_transparent: true }`. En Windows, GPUI quita la barra del sistema y conserva los bordes para redimensionar, la sombra y el título en la barra de tareas. Solo se lee al abrir la ventana (Windows no deja cambiarlo después); un cambio posterior se ignora con un aviso.
- Prop `windowControl` en `div` (`drag`, `minimize`, `maximize`, `close`), con `window_control_area` de GPUI. Windows trata esas zonas como partes de su propia barra (`HTCAPTION`, `HTMINBUTTON`…), así que arrastrar, Snap, el doble clic para maximizar y los botones son nativos y no pasan por el protocolo. `close` termina en `windowClosed` como siempre.
- Problema encontrado en la prueba: un `Button` dentro de la zona `drag` no recibía clics y arrastrarlo movía la ventana, porque GPUI responde con el área de cualquier elemento bajo el cursor. Solución: dentro de una zona `windowControl`, `Button`, `Checkbox`, `div` con `click` y una zona `windowControl` anidada usan `occlude()`, que tapa la zona de atrás; `Input` no lo implementa, así que se envuelve en un `div` que sí, y ese `div` recibe sus clases. Fuera de esas zonas no cambia nada.
- Clientes: `frameless` en `WindowOptions` y `windowControl` en las props de `div` (Node y React); en Python funcionan sin cambios (`app.window(frameless=True)`, `app.div(windowControl="drag")`).
- `examples/frameless.ts`: barra propia con icono, título y botones de minimizar, maximizar y cerrar con los iconos `window-*`.
- Verificado con el mouse en ventana real: arrastrar la barra mueve la ventana exactamente lo arrastrado y arrastrar el contenido no; maximizar con el botón y restaurar con doble clic en la barra; minimizar; cerrar emite `windowClosed` y el host termina; un botón normal sigue funcionando. Con un `Button`, un `div` clicable y un `Input` dentro de la zona `drag`: los tres reciben clics y texto, arrastrar desde el botón no mueve la ventana y arrastrar desde la parte vacía de la barra sí.
- Tests: casos en `conformance/window.txt` (`frameless` mal tipado, `windowControl` válido, inválido y en un tipo que no lo tiene).
- No probado: el menú de Snap Layouts al pasar el mouse sobre `maximize` (Windows 11) y un `windowControl` anidado dentro de `drag` (el ejemplo los pone uno al lado del otro).

### Fase 19 — Varias ventanas

Tercer paso de la §8, con el diseño acordado antes de empezar.

**Criterios de aceptación:** una app abre, actualiza y cierra varias ventanas, cada una con su raíz; los eventos y el texto de los Input funcionan en todas; `windowClosed` dice qué ventana cerró el usuario y el host termina solo después de la última; los clientes de una sola ventana siguen funcionando sin cambios.

**Estado:** completada.
- Protocolo (PROTOCOL.md §3 y §6): `window` acepta `id` (sin `id` es la ventana 1); `closeWindow` la cierra en el `commit` sin enviar `windowClosed`; `windowClosed` lleva el `id` y solo se envía cuando cierra el usuario. Una raíz no puede ser de dos ventanas (`invalid_operation`). El tema es de toda la app: `theme`/`themeSet` se guardan aparte (`ThemeSpec`) y se aplican a todas.
- Host: `Tree` guarda las ventanas en un `BTreeMap`; `render::sync_windows` abre las que tienen raíz, actualiza las que cambiaron y cierra con `remove_window()` las que el cliente quitó. `Windows` (compartido con un `Rc<RefCell>`) sabe qué ventana de GPUI es cada id; `render::window_closed` (de `on_window_closed`) solo reporta las que siguen en esa lista, y como `sync_windows` saca de la lista las que cierra el cliente antes de cerrarlas, esas no generan `windowClosed`. El host sale con `cx.quit()` cuando el usuario cierra la última. Encontrado en la prueba: en Windows, GPUI cierra la app sola cuando no quedan ventanas (`QuitMode` por defecto), así que cerrar la última con `closeWindow` terminaba el host; ahora usa `QuitMode::Explicit` y solo sale por la X de la última ventana o cuando se cierra stdin. Verificado: cerrar todas con `closeWindow` deja el host vivo y una nueva `window` vuelve a abrir; cerrar stdin sale con código 0; la X de la última ventana sale.
- Cada ventana tiene su `RootView` con su id. Antes, el primer `RootView` que leía los `value` impuestos a los Input los consumía (`take_value_writes`); ahora el árbol cuenta los `commit` y cada vista aplica los de su ventana una sola vez por `commit`, porque la sesión también avisa entre commits (un clic en un checkbox) y reaplicarlos borraría lo que el usuario escribió.
- `simulate` cierra una ventana por `id`; las transcripciones cubren cerrar la 2 (el host sigue) y después la 1 (sale), más `conformance/windows.txt` (ids, raíz compartida, `id` 0, `closeWindow` inexistente o mal tipado, tema desde cualquier ventana).
- Clientes:
  - Node: `app.createWindow(opciones)` devuelve un `AppWindow` con `set`, `close` y `on("closed")`; `app.on("windowClosed", ({ id }) => …)`. El cliente lleva la cuenta de las ventanas abiertas para saber cuándo la salida del host es esperada.
  - Python: `app.create_window(...)` devuelve un `Window` con `set`, `close` y `on_closed`; `on_window_closed` ahora recibe el id.
  - React: `<Window title=… onClose=…>` abre una ventana mientras está montado y la cierra al desmontarse, también si se desmonta un ancestro (`detachDeletedInstance`, que corre después del `commit` de React, así que el cierre va en un `commit` propio). Sus hijos se dibujan en esa ventana, no en el padre. Atajo: un nodo insertado justo antes de un `<Window>` hermano va al final.
- `examples/windows.ts`: ventana principal que abre "App settings" (el título "Settings" choca con la app Configuración de Windows al buscar la ventana por título). Verificado con mouse y teclado: abrir; escribir en las dos ventanas; el modo oscuro desde ajustes repinta las dos; cerrar desde la app (sin `windowClosed`); reabrir (id nuevo) y cerrar con la X (`windowClosed 3`, el host sigue); cerrar la principal con ajustes abierta (el host sigue y ajustes funciona) y después ajustes (`windowClosed 2`, el host sale).
- Al cerrar una ventana, GPUI escribe en stderr `window not found` e `Invalid window handle`, igual con la X que con `closeWindow`; son internos de GPUI, como los que ya anotaba la fase 1.
- Tests: host (unitarios y transcripciones), Node (`createWindow` contra el host), React (abrir, actualizar, `onClose`, cerrar al desmontar y al desmontar un ancestro) y Python (`create_window`).
- No probado en ventana real: `<Window>` de React (sí sin ventana, con el host real) y Python con varias ventanas. Fuera de este paso: posición inicial, ventanas modales o hijas, eventos de foco y movimiento.

### Fase 20 — Menús

Cuarto paso de la §8, con el diseño acordado antes de empezar (opción "botones con `dropdown_menu`" para la barra).

**Criterios de aceptación:** un `div` abre un menú contextual con clic derecho y una `MenuBar` abre sus menús; los dos tienen iconos, separadores, ítems deshabilitados y marcados y submenús; elegir un ítem avisa al cliente con su id.

**Estado:** completada.
- En Windows, GPUI no dibuja barra de menú nativa (`set_menus` solo guarda la lista) y el `AppMenuBar` de gpui-component funciona con *Actions* de GPUI, que son tipos de Rust fijos. Por eso los dos menús usan el `PopupMenu` de gpui-component con un `on_click` por ítem.
- Protocolo (PROTOCOL.md §10): un solo formato de ítems (`id` + `label`, submenú con `items`, `{"separator":true}`, opcionales `icon`, `disabled`, `checked`). Prop `contextMenu` en `div` (con `context_menu` de gpui-component) y nodo `MenuBar` con `menus` (un botón `ghost` por menú, con `dropdown_menu`). Elegir un ítem envía el evento opt-in `menu` con el id en `value`; no lleva la ventana porque el nodo ya la identifica. La lista se valida entera (`invalid_props` con el ítem que falla en el mensaje).
- Un submenú deshabilitado se dibuja como ítem deshabilitado: gpui-component no deja deshabilitar un submenú.
- Dentro de una zona `windowControl`, la `MenuBar` usa `occlude()`, como `Button`.
- `simulate` elige un ítem (`event: "menu"`, `value`: id), también dentro de submenús; uno deshabilitado no hace nada y uno inexistente es `invalid_operation`. Transcripción `conformance/menus.txt`.
- Clientes: `app.menuBar()` y `contextMenu` en Node (evento `menu` con `{ item }`), `<MenuBar menus onMenu>` y `contextMenu`/`onMenu` en `Div` en React, `app.menu_bar()` y `contextMenu=` en Python (el handler recibe el id). Un test por cliente.
- `examples/menus.ts`: barra con File/Edit/View (submenú, ítem deshabilitado, "Word wrap" marcado que la app alterna) y un recuadro con menú contextual; `--frameless` pone la barra dentro de la barra de título propia.
- Verificado con el mouse en ventana real: File abre con iconos y separador; el submenú "Open recent" se abre al pasar el mouse y elegir un ítem lo informa y cierra el menú; Undo deshabilitado no hace nada; alternar "Word wrap" cambia la marca al volver a abrir View (el menú se reconstruye con los `menus` nuevos); un clic afuera cierra el menú; File → Quit termina la app con código 0. Menú contextual: se abre en el puntero con Paste deshabilitado; el submenú "Sort by" funciona (gpui-component lo abrió hacia la izquierda); un clic afuera lo cierra y el clic derecho fuera del recuadro no abre nada. Con `--frameless`: los botones de la barra reciben clics dentro de la zona `drag` y sus ítems también; arrastrar la parte vacía mueve la ventana exactamente lo arrastrado y arrastrar desde File no la mueve; el clic derecho sobre la zona `drag` no muestra el menú de sistema de Windows; la X emite `windowClosed` y el host sale.
- Fuera de este paso: atajos de teclado en los ítems, flechas ←/→ entre menús y cambiar de menú al pasar el mouse con uno abierto, menú nativo de macOS. No probado: un `contextMenu` en un `div` que además es zona `drag`.

### Fase 21 — Bandeja del sistema

Quinto paso de la §8, con el diseño acordado antes de empezar (incluida la dependencia nueva).

**Criterios de aceptación:** la app muestra un icono en la bandeja con tooltip y menú; el clic y los ítems del menú llegan al cliente; con bandeja, cerrar la última ventana no termina la app y se puede volver a abrir la ventana desde la bandeja; quitar la bandeja sin ventanas abiertas termina la app.

**Estado:** completada.
- GPUI no trae bandeja: se usa `tray-icon` 0.26 (de Tauri), que en Windows agrega `muda` (menús nativos), `windows-sys`, `crossbeam-channel` y poco más. El icono se lee con `image` 0.25, que GPUI ya usaba (mismas features, nada nuevo que descargar). Funciona en el hilo principal sin hilos extra: `tray-icon` crea una ventana oculta y el bucle de GPUI despacha los mensajes de todas las ventanas del hilo. Sus handlers de eventos corren ahí mismo y escriben directo en stdout.
- Protocolo (PROTOCOL.md §11): operación `tray` (`icon`, `tooltip`, `menu`; se mezclan como en `window`) y `removeTray`; eventos `{"type":"tray","event":"click"}` y `{"type":"tray","event":"menu","value":id}`, siempre enviados. El menú usa el formato de la §10 y se valida igual (`invalid_json` si está mal).
- El menú es nativo de Windows: no sigue el tema, no dibuja iconos de ítems, y un ítem con `checked` cambia su marca solo al elegirlo (comportamiento de Windows). Cada mensaje `tray` reconstruye todo (`TraySpec::revision`), así el cliente lo vuelve a sincronizar y un icono cambiado en disco se relee. `&` se escapa para que no sea un atajo de teclado.
- Ciclo de vida: con bandeja, cerrar la última ventana manda `windowClosed` y el host sigue; quitar la bandeja sin ventanas abiertas lo termina (también en `--no-window`). Los clientes ya no dan por esperada la salida del host al cerrar la última ventana si hay bandeja, y sí al quitar la bandeja sin ventanas.
- Atajo: el icono se pasa como un solo bitmap de 32 px y Windows lo escala al tamaño de la bandeja.
- `simulate` admite `trayClick` y `trayMenu`; transcripción `conformance/tray.txt` (errores, eventos, mezcla de campos, el host sigue tras cerrar la última ventana y termina al quitar la bandeja; la última línea comprueba que de verdad salió).
- Clientes: Node `app.tray()`, `app.removeTray()` y `app.on("tray", …)`; Python `app.tray()`, `app.remove_tray()` y `on_tray(event, item)`; React `<Tray icon tooltip menu onClick onMenu>`, que existe mientras está montado. Encontrado en el test de React: `detachDeletedInstance` se llama dos veces por instancia (se enviaba `removeTray` dos veces; ahora tiene una marca, como `Window`), y un `menu` escrito inline se reenviaba en cada render (ahora se compara por valor).
- `examples/tray.ts` con `examples/tray-icon.png` (32×32, generado): cerrar la ventana deja la app en la bandeja y el clic en el icono o "Show window" la vuelve a abrir.
- Verificado en Windows 11 con el mouse: el icono aparece (Windows 11 lo pone en los iconos ocultos); la X cierra la ventana y el host sigue; el clic izquierdo la reabre; el clic derecho abre el menú nativo con marca, submenú, ítem deshabilitado y separador; elegir "Away" en el submenú llega como `status:away`; "Notifications" alterna la marca; el ítem deshabilitado no envía nada; con la ventana cerrada, "Quit" quita la bandeja, la app termina con código 0 y el icono desaparece. Cerrar stdin con la bandeja puesta también la quita (comprobado con UI Automation).
- Fuera de este paso: varias bandejas, doble clic, notificaciones, y ocultar una ventana sin cerrarla.

### Fase 22 — Animaciones

Sexto y último paso de la §8, con el diseño acordado antes de empezar (con transiciones también en `hover:`).

**Criterios de aceptación:** un `Icon` gira con `animate-spin`; cualquier nodo late con `animate-pulse`; con `transition`, los colores y la opacidad cambian de forma gradual al cambiar las clases, el tema o el `hover:`; sin nada en movimiento, el host no gasta CPU.

**Estado:** completada (verificada en la sesión de pruebas del 2026-10-18).
- Solo clases nuevas en la §8 (`opacity-*`, `transition`, `duration-*`, `ease-*`, `animate-spin`, `animate-pulse`); el protocolo y los clientes no cambian, porque `class` sigue siendo texto.
- GPUI no tiene transiciones: cambiar un estilo salta al valor nuevo. `animation.rs` las calcula como función pura del tiempo: cada ventana guarda por nodo el valor de partida, el de llegada y el inicio (`Transition`); un objetivo nuevo a mitad de camino arranca desde lo que se ve; un nodo nuevo no anima. Los colores se interpolan en RGBA (por el tono, de rojo a azul se pasaría por verde); un fondo sin clase se funde como transparente y un color de texto o borde sin clase cambia de golpe (no se sabe desde dónde partir). Los nodos que desaparecen sueltan su estado.
- `hover:` con `transition`: el `hover` de GPUI aplica el estilo al pintar y no se puede animar, así que en esos `div` el host sigue el puntero con `on_hover` y deja de registrar el `hover` de GPUI (si estuvieran los dos, saltaría al color final).
- `animate-spin` usa la rotación de los SVG (solo `Icon`; en otros tipos avisa una vez y se ignora). `animate-pulse` multiplica la opacidad. GPUI no tiene `transform` para `div`, por eso quedaron fuera `animate-ping` y `animate-bounce`.
- La ventana pide otro cuadro (`request_animation_frame`) solo mientras algo se mueve. Medido: con un spinner y un `animate-pulse`, el host gasta ~450 ms de CPU cada 3 s (~15 % de un núcleo); con nodos `transition` en reposo, 31 ms cada 3 s. Atajo anotado en el código: cada cuadro reconstruye toda la ventana, así que un spinner sobre un árbol grande lo redibuja entero a 60 fps.
- Tests: unitarios de las clases (válidas e inválidas, `ease-in-out` antes del caso `prefijo-número`) y de la interpolación (mitad de camino en RGBA, final, objetivo nuevo a mitad de camino, nodo nuevo, valores sin clase, pulse). Sin transcripción de conformidad: una transcripción no ve el render y una clase desconocida solo avisa en stderr.
- `examples/animations.ts`: spinner, esqueleto con pulse, filas con `hover:` de 1 s, 150 ms y 500 ms lineal, una caja que cambia de color y opacidad, y modo oscuro con `transition` en la raíz.
- Verificado en ventana real (capturas y color de un píxel medido en el tiempo): el spinner gira; el esqueleto late y sus hijos se funden con él (la opacidad de un `div` alcanza a sus hijos); el `hover:` de 1 s va de `#ffffff` a `#f5f5f5` en ~900 ms con easing y vuelve igual al salir; la caja pasa de casi negro a rojo al 30 % en ~700 ms; el modo oscuro hace la transición de la raíz en ~450 ms.

### Fase 23 — Formularios

Primer paso de la §9, con el diseño acordado antes de empezar (solo `change` en el `Slider`).

**Criterios de aceptación:** `Switch`, `RadioGroup`, `Select` (con búsqueda), `Slider` y `Textarea` se crean, se actualizan y avisan sus cambios; los cuatro primeros tienen los modos sincronizado y controlado de `Checkbox`; los valores impuestos por el cliente se ven.

**Estado:** completada (verificada en la sesión de pruebas del 2026-10-18).
- Protocolo (PROTOCOL.md §5, §6 y §9): cinco tipos de nodo nuevos. `RadioGroup` es un solo nodo con una lista de `options` (como los menús), no un nodo por opción, así el grupo tiene un único valor. El `value` del evento puede ser un número (solo `Slider`); los clientes lo entregan como `number`/`float`.
- `Switch` y `RadioGroup` funcionan como `Checkbox`: el estado está en las props y vuelve por `on_click` (helper común `value_chosen`, hermano de `checkbox_clicked`). Volver a elegir la opción marcada no es un cambio.
- `Select` y `Slider` tienen estado propio en GPUI (una entidad por nodo, como `Input`). Su valor vive en el árbol (`value`, como `checked`) y `sync` hace que la entidad lo muestre; eso mismo revierte el cambio de un control `controlled` hasta que el cliente lo confirma. Un `Select` se reconstruye cuando cambian `options` o `searchable` (son de construcción); el rango de un `Slider` se aplica reconstruyendo su estado con los métodos de construcción, que son los únicos que hay. Un `max` que no supera a `min` se corrige para no dividir por cero, y `value` se recorta al rango. Los números de GPUI (`f32`) se envían con su forma corta (`0.3`, no `0.30000001192092896`).
- `Textarea` reusa el código de `Input`, ahora genérico sobre el modo de texto de gpui-component (`InputSlot<M>`). Siempre sincronizado: en el modo controlado, cada corrección lleva el cursor al final, lo que no sirve para texto de varias líneas (se ignora `controlled` aunque llegue).
- Dentro de una zona `windowControl`, los controles que no tienen `occlude` (`Switch`, `Select`, `Slider`, `Textarea`) van en un `div` que sí lo tiene y toma sus clases, como `Input`; `RadioGroup` va siempre en un `div` que toma sus clases (el grupo arma su propio layout).
- `simulate` cubre los cinco (opciones inexistentes, deshabilitadas o repetidas, números fuera de rango o de otro tipo); transcripción `conformance/forms.txt` con los dos modos y las props inválidas.
- Clientes: Node `app.switch()`, `radioGroup()`, `select()`, `slider()`, `textarea()` con tipos por nodo; Python `switch()`, `radio_group()`, `select()`, `slider()`, `textarea()`; React `<Switch>`, `<RadioGroup>`, `<Select>`, `<Slider>`, `<Textarea>` (pasar `value` o `checked` los vuelve controlados, como en React DOM). Un test por cliente.
- `examples/forms.ts`: los cinco controles, con el volumen en modo controlado (la app lo redondea a múltiplos de 10) y una línea que muestra el estado.
- Verificado sin mouse, en una ventana real: el host construye los cinco controles, acepta valores y opciones nuevas impuestos por el cliente y sale con código 0, sin errores ni pánicos. Falta probar con el mouse: abrir el `Select` y buscar, arrastrar el `Slider` (y ver el redondeo del modo controlado), escribir varias líneas en el `Textarea`, la opción deshabilitada del `RadioGroup` y el `Switch`.
- Verificado con mouse y teclado: el `Switch` se apaga; "Large" queda marcado y la opción deshabilitada no responde; el `Select` filtra al buscar "ur" y elegir Uruguay informa `ur`; arrastrar el `Slider` controlado lo deja en 60 (el valor redondeado que confirma la app); el `Textarea` agrega una línea con Enter y el estado muestra `first line\nsecond line`.
- Fuera de este paso: selección múltiple (`Combobox`), grupos dentro de `Select`, `Slider` de rango, selector de fecha y `NumberInput`.

### Fase 24 — Teclado

Paso 2 de la §9, primera de sus dos fases (la segunda es la 25), con el diseño acordado antes de empezar.

**Criterios de aceptación:** la app registra atajos y los recibe con la ventana; los ítems de `MenuBar` muestran su atajo y se eligen con él; un `div` recibe teclas y doble clic; el cliente puede dar el foco a un control.

**Estado:** completada (verificada en la sesión de pruebas del 2026-10-18).
- Protocolo (PROTOCOL.md §12): `shortcuts` (lista de la app, reemplaza la anterior) y `focus` se aplican en el `commit`; mensaje `shortcut` con la tecla y la ventana; eventos opt-in `keyDown` (`div` e `Input`) y `doubleClick` (`div`); `shortcut` en los ítems de menú. Las teclas usan el formato de GPUI (`ctrl-shift-s`), validado con `Keystroke::parse`.
- Encontrado al revisar GPUI: sin nada enfocado, una tecla va al nodo raíz de la ventana, que es el `Root` de gpui-component y no nuestra vista. Por eso cada `RootView` tiene su `FocusHandle`, lo toma al abrir y lo recupera en el render cuando nada tiene el foco, y escucha las teclas en fase de captura: así un atajo gana sobre el control enfocado y `stop_propagation` evita que el control la use también.
- Atajos de `MenuBar`: el host busca en la ventana el ítem habilitado con esa tecla (`menu::find_shortcut`) y envía su evento `menu`, sin abrir el menú. El atajo se dibuja con el `Kbd` de gpui-component a la derecha del texto (`PopupMenuItem::element`). En la bandeja se pasa como acelerador de `muda`, que solo lo muestra.
- `keyDown` en `div`: el nodo tiene su `FocusHandle` (con `tab_stop`), toma el foco al hacer clic salvo que el clic haya enfocado algo de adentro, y avisa en fase de captura. En `Input`, un `div` envoltorio toma las clases y escucha (el `Input` no puede). `doubleClick` usa `click_count()` del clic de GPUI.
- `focus` espera a que la ventana que muestra el nodo lo aplique en su `sync` (Input, Textarea o `div` con `keyDown`). Encontrado al revisar (fase 25): al abrir una ventana, la vista aplicaba el pedido y después le devolvía el foco a la raíz, así que `focus()` en el mismo `commit` que abre la ventana no hacía nada; ahora la raíz toma el foco primero (sin probar aún en ventana real).
- `simulate` admite `key` (atajos y atajos de menú), `keyDown` y `doubleClick`; transcripción `conformance/keyboard.txt` (modificadores exactos, ítem deshabilitado, reemplazo de la lista, teclas inválidas, `focus` inválido).
- Clientes: Node `app.shortcuts()`, `app.on("shortcut")`, `node.focus()`, eventos `keyDown` (`{ key }`) y `doubleClick`; Python `shortcuts()`, `on_shortcut`, `node.focus()`; React `onKeyDown`, `onDoubleClick` y `ref` (que da el nodo: `ref.current.focus()`; se agregó `ref` al tipo de los componentes). Un test por cliente.
- `examples/keyboard.ts`: menú con atajos, atajo de la app, lista con flechas, Enter y doble clic, y buscador que se vacía con Escape.
- Verificado sin teclado: el ejemplo abre en ventana real sin errores. Falta probar con el teclado: atajos con y sin un `Input` enfocado, Tab hacia la lista, flechas, doble clic y `focus()` desde Ctrl+F.
- Verificado con teclado y mouse: el menú Edit muestra Ctrl+F y F5 a la derecha; sin nada enfocado, Ctrl+N y F5 eligen sus ítems y Ctrl+Shift+D llega como atajo de la app; Ctrl+F enfoca el buscador (`focus()`) y se puede escribir; Ctrl+N con el `Input` enfocado elige el ítem sin escribir la "n"; Escape lo vacía; clic en la lista, flechas, Enter y doble clic; Tab pasa del buscador a la lista; Ctrl+Q cierra la app.
- Fuera de este paso: atajos globales del sistema, eventos de tecla soltada, eventos de foco y blur.

### Fase 25 — Diálogos nativos y sistema

Paso 2 de la §9, segunda de sus dos fases.

**Criterios de aceptación:** la app abre los diálogos de abrir y guardar del sistema y recibe lo elegido; lee y escribe texto en el portapapeles; abre URLs y archivos con su programa; recibe archivos soltados sobre un `div`.

**Estado:** completada (verificada en la sesión de pruebas del 2026-10-18).
- Protocolo (PROTOCOL.md §13): acciones `openDialog`, `saveDialog`, `readClipboard`, `writeClipboard`, `openUrl`, `openPath`, que se ejecutan al llegar (no en el `commit`): `handle_line` las devuelve como `Flow::Action` y el bucle principal las corre con la `App` de GPUI (`system.rs`). Las que contestan llevan un `request` del cliente y reciben `result` (`paths`, `path` o `text`, `null` al cancelar). Evento opt-in `drop` en `div` con el campo nuevo `paths`.
- Encontrado en el test de Node: si el host rechaza una acción con `request`, nunca llega su `result` y la promesa quedaba esperando para siempre. Ahora el `error` lleva el `request` (también cuando el JSON tiene un campo mal tipado: el host lo saca de la línea cruda) y los clientes fallan la promesa o lanzan la excepción.
- Los diálogos usan `prompt_for_paths` y `prompt_for_new_path` de GPUI y contestan en una tarea aparte, así la app sigue respondiendo mientras están abiertos. Un diálogo que falla contesta `null` y avisa en stderr. Sin `directory`, el de guardar empieza en el directorio de trabajo del host.
- Con `--no-window` los diálogos y el portapapeles contestan `null`, así las transcripciones cubren la validación (`conformance/system.txt`, que incluye `drop`).
- Clientes: Node `await app.openDialog()`, `saveDialog()`, `readClipboard()` (promesas; varias a la vez; rechazadas si el host sale) y `writeClipboard()`, `openUrl()`, `openPath()`; Python `open_dialog()`, `save_dialog()`, `read_clipboard()` (esperan la respuesta y mientras tanto siguen despachando eventos; guardan respuestas de otros pedidos) y las otras tres; React `onDrop`. Un test por cliente.
- `examples/system.ts`: botones para cada acción y una zona para soltar archivos.
- Verificado sin mouse: el ejemplo abre en ventana real sin errores. Falta probar: los dos diálogos (elegir y cancelar), copiar y pegar con otra app, abrir la URL y el archivo, y soltar archivos desde el Explorador.
- Verificado en la sesión de pruebas del 2026-10-18: el diálogo de abrir es el nativo, con el botón "Pick" pedido, devuelve la ruta escrita y `null` al cancelar con Escape; el de guardar empieza en el directorio de trabajo con `notes.txt` sugerido y devuelve la ruta (no crea el archivo: eso es de la app); el portapapeles funciona en los dos sentidos con PowerShell, también con `ñ` y `✓`; `openPath` llega a `ShellExecute("open")` (en este equipo `.md` no tiene app confirmada y Windows mostró su selector, igual que con doble clic en el Explorador). Con la sesión bloqueada Windows niega el portapapeles (`Access is denied`) y el host contesta `null` sin caerse.
- Soltar archivos: un arrastre OLE simulado desde PowerShell no funcionó ni contra un formulario de prueba que acepta archivos (la falla era de la simulación); el usuario lo probó a mano arrastrando desde el Explorador a `examples/system.ts` y funciona. `openUrl` no se probó (usa el mismo `ShellExecute`).
- Fuera de este paso: filtros por tipo de archivo (GPUI no los tiene), imágenes en el portapapeles, "mostrar en la carpeta" y arrastrar archivos hacia afuera de la app.

### Fase 26 — Diálogos, notificaciones y tooltips

Paso 3 de la §9, con el diseño acordado antes de empezar (opción "contenido libre" para el `Dialog`).

**Criterios de aceptación:** un nodo `Dialog` muestra un modal con cualquier contenido y avisa OK o Cancelar; la app muestra toasts (también en el centro de notificaciones de Windows) y sabe cuándo se hace clic en uno; `div` y `Button` muestran un tooltip.

**Estado:** completada (verificada en la sesión de pruebas del 2026-10-18).
- Protocolo (PROTOCOL.md §14): nodo `Dialog` (`open`, `title`, `okText`, `cancelText`, `width`, `class` para el cuerpo; eventos `ok` y `cancel`), acción `notify` (como las de la §13, con `id` opcional para el evento `notification` y `system` para el centro de notificaciones) y prop `tooltip` en `div` y `Button`.
- `Dialog` usa `window.open_dialog` de gpui-component (fondo, título, Escape, X, foco atrapado). El cuerpo es una vista (`dialog::Body`) que le pide a la `RootView` de la ventana que dibuje los hijos del nodo, así que `Input`, estilos y eventos funcionan igual que en el resto de la ventana; se redibuja cuando cambia la sesión. El nodo `Dialog` no dibuja nada en su lugar del árbol. El pie se arma con `DialogAction` y `DialogClose` (un `Dialog` común no trae botones; eso es de `AlertDialog`).
- Al revisar gpui-base: Escape, el clic afuera y Cancelar llaman a `on_cancel` y después a `on_close`; OK llama a `on_ok` y después a `on_close`. Por eso solo hay eventos `ok` y `cancel` (un `close` aparte los duplicaría). Al responder, el host pone `open: false` (como un control sincronizado); si el cliente lo cierra con `open: false`, la vista lo olvida antes de cerrarlo, así no se informa como respuesta del usuario.
- Encontrado en la prueba en ventana real: un `Dialog` abierto en el mismo `commit` que abre la ventana hacía caer el host (`component window state is missing`), porque se abría mientras se construía la vista, antes de que gpui-component la envuelva en su `Root`. Ahora abrir o cerrar el diálogo y aplicar `focus` se hacen en un paso diferido (`defer_in`), con la ventana ya armada; así además un `focus` sobre un `Input` del diálogo llega después de que el diálogo existe.
- Atajo anotado en el código: en el cuerpo de un `Dialog` no se conservan las transiciones ni el seguimiento del `hover:` (cada pasada empieza en reposo).
- `notify` busca la ventana del cliente (`Windows::handle`); una ventana que no está abierta es `invalid_operation`. Toasts con el mismo `id` se reemplazan (`id1` de gpui-component).
- `simulate` admite `ok` y `cancel` en un `Dialog` abierto; transcripción `conformance/overlays.txt`.
- Clientes: Node `app.dialog()`, `app.notify()` y `app.on("notification")`; Python `dialog()`, `notify()` y `on_notification`; React `<Dialog open onOk onCancel>` con hijos (al responder, el host lo cierra: el estado se actualiza en `onOk`/`onCancel`). `tooltip` en `div` y `Button` en los tres. Un test por cliente.
- `examples/overlays.ts`: diálogo de renombrar con un `Input` (Enter también acepta), toasts de cada tipo, uno con clic, uno también en Windows, y tooltips.
- Verificado sin pantalla, en ventana real: abrir, cambiar el título, cerrar y reabrir el diálogo desde el cliente, abrirlo junto con la ventana, y los toasts, sin errores ni pánicos. Falta ver: el diálogo y su contenido, OK, Cancelar, Escape, la X y el clic afuera, el foco en el `Input` del diálogo, los toasts (y el de Windows) y los tooltips.
- Verificado en ventana real: el diálogo aparece con fondo oscurecido, título, cuerpo, X y botones; `focus()` deja el cursor en su `Input`; Enter, el botón OK, Escape, Cancelar, la X y el clic afuera responden como corresponde y se puede reabrir; los toasts de cada tipo aparecen y se ocultan solos (gpui-component pausa el cierre mientras el mouse está encima); el clic en un toast con `id` llega a la app; los tooltips de `div` y `Button` aparecen.
- Encontrado en la sesión: los toasts con `system: true` no llegaban a Windows, porque GPUI necesita una identidad de app (`set_app_identity`, un AppUserModelID) y solo lo avisaba en stderr. El host ahora registra `gpui-bridge` al arrancar (atajo: una identidad fija para todas las apps) y el toast aparece en el centro de notificaciones de Windows.
- Fuera de este paso: varios diálogos apilados, diálogos sin OK o con más botones, botones dentro de un toast y tooltips con contenido.

### Fase 27 — Lista virtualizada, `Table` y `Tabs`

Paso 4 de la §9, con el diseño acordado antes de empezar: filas de la lista como nodos hijos, celdas de la tabla como texto en props y `Tabs` como barra sola.

**Criterios de aceptación:** una lista de 10 000 filas hechas con nodos se desplaza con fluidez y solo dibuja las visibles; una tabla de 10 000 filas de datos se selecciona con mouse y teclado, informa doble clic y deja que la app ordene; una barra de pestañas informa la elegida.

**Estado:** completada (verificada en ventana real el 2026-10-18).
- Protocolo (PROTOCOL.md §15): nodos `VirtualList` (`rowHeight`; cada hijo es una fila), `Table` (`columns`, `rows`, `selected`, `stripe`, `bordered`, `controlled`; eventos `change` con el índice o `null`, `doubleClick` con el índice y `sort` con `{column, direction}`) y `Tabs` (`tabs` con el formato de `options`, `value`, `variant`; evento `change`). `focus` también acepta un `Table`.
- `VirtualList` usa `v_virtual_list` de gpui-component con la `RootView` como vista: la lista le pide las filas visibles y la vista las arma en una pasada aparte, igual que el cuerpo del `Dialog` (las dos comparten ahora `RootView::render_cx`). El handle de scroll se guarda por nodo en la `RootView`: `v_virtual_list` crea uno nuevo en cada llamada y sin eso el scroll volvía a 0 en cada cuadro. Cada fila va dentro de una grilla de una celda, que la estira al ancho de la lista y a `rowHeight` (sin eso la fila medía lo que su contenido).
- `Table` usa `DataTable` (virtualizada, con columnas redimensionables y flechas). El delegate lee `columns` y `rows` del árbol cuando dibuja, así una tabla grande no se copia en cada sync; el estado solo se refresca si cambian las columnas. Mover columnas y seleccionar columnas quedan apagados (las celdas siguen el orden de `columns`). `DataTable` deja la alineación al delegate, así que celdas y encabezados se alinean ahí. Elegir una fila desde la app (`selected`) no se informa de vuelta como cambio del usuario, como en `Select`.
- Ordenar es de la app: gpui-component cicla ninguno → desc → asc → ninguno y el host solo informa la columna y la dirección. `simulate` recibe la dirección explícita, porque el ciclo vive en el estado de la ventana.
- `Tabs` es el `TabBar` de gpui-component, armado como el `RadioGroup` (el estado vive en las props). El contenido lo muestra la app; en `examples/data.ts` se mueve el panel elegido a la ventana y el otro espera en un `div` suelto.
- `simulate` admite `change`, `doubleClick` y `sort` en `Table` y `change` en `Tabs`; transcripción `conformance/data.txt`.
- Clientes: Node `app.virtualList()`, `app.table()`, `app.tabs()` (el payload de `sort` es `{column, direction}`); Python `virtual_list()`, `table()`, `tabs()`; React `<VirtualList>`, `<Table onChange onDoubleClick onSort>` (pasar `selected` la hace controlada) y `<Tabs>`. Un test por cliente.
- `examples/data.ts`: `Tabs` para pasar de una `VirtualList` de 10 000 filas (icono, texto, `hover:` y clic) a una `Table` de 10 000 filas ordenable por nombre y tamaño.
- Verificado en ventana real: la lista se desplaza con la rueda (con su barra de scroll), el clic en cualquier punto de la fila llega, y el `hover:` se ve; la tabla muestra columnas alineadas, ordena por tamaño descendente con un clic, selecciona con clic, flechas (la tabla toma el foco) y Escape, e informa el doble clic; las pestañas cambian el panel en los dos sentidos. Una tabla controlada que confirma cada fila no salta al hacer clic lejos de la seleccionada (la confirmación llega antes de dibujar y reemplaza el desplazamiento de la reversión). Achicar `rows` por debajo de `selected` no manda ningún `change` (lo encontró la revisión: el host informaba `null` como si lo hubiera hecho el usuario; ahora la ventana y `simulate` ignoran un `selected` fuera de rango). En reposo con todo cargado: ~1 % de CPU y ~130 MB.
- Atajo anotado en el código: las filas de la lista no conservan transiciones ni el seguimiento del `hover:` animado entre pasadas (como el cuerpo del `Dialog`).
- Fuera de este paso: celdas con otro contenido que texto, selección de celdas o columnas, mover columnas, menú contextual por fila, cargar más al llegar al final, filas de distinto alto, `scrollTo` desde la app y pestañas que se cierran o reordenan.

### Extra — Cabos sueltos (2026-10-19)

Cierre de los puntos que quedaron sin verificar en las fases 17–27, acordado con el usuario: se
hacen de a uno, en este orden, con revisión entre cada uno, y después se sigue con el backlog de
[GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md) §8. Ninguno cambia el protocolo, salvo que haga falta
(se propone antes de programarlo).

| # | Cabo suelto | Estado |
|---|---|---|
| 1 | `focus()` en el mismo `commit` que abre la ventana (fase 24) | hecho |
| 2 | `openUrl` en ventana real (fase 25) | hecho |
| 3 | Transiciones y `hover:` en las filas de `VirtualList` y el cuerpo del `Dialog` (fases 26 y 27) | hecho |
| 4 | Cliente HTTP para `Image` con URL remota (fase 17) | decidido: sin HTTP en el host (ver "Medición") |

**1. `focus()` en el mismo `commit` que abre la ventana — hecho (verificado en ventana real el 2026-10-19).**

- El arreglo ya estaba desde la fase 25: la vista toma el foco de la raíz al construirse y aplica el `focus` pedido en un paso diferido (`defer_in`), con la ventana ya armada.
- Lo que faltaba era un ejemplo que lo ejercitara. `examples/windows.ts` ahora pide `note.focus()` en el commit que abre la ventana principal y `name.focus()` en el commit que abre la de ajustes.
- Verificado en una ventana real: las ventanas "Main" y "App settings" quedan con su campo enfocado y el texto escrito **sin hacer clic** cae en el campo (las dos).
- Sin cambios de protocolo ni del host: solo el ejemplo.

**2. `openUrl` en ventana real — hecho (verificado en ventana real el 2026-10-19).**

- Quedó sin probar en la fase 25 (se había verificado `openPath`, que usa el mismo `ShellExecute`).
- Verificado con `examples/system.ts`: el botón "Open website" abre el navegador en `https://github.com/zed-industries/zed`; tras el clic el host no escribió salida nueva en stderr (con `--debug`; al arrancar ya había registrado el error conocido `0x887A002D` de DirectX) y siguió respondiendo.
- Sin cambios de código.

**3. Transiciones y `hover:` animado en las filas de `VirtualList` y en el cuerpo de un `Dialog` — hecho (verificado en ventana real el 2026-10-19).**

- Causa: `render_rows` y `render_dialog_body` armaban su pasada con las transiciones vacías y un set de hover nuevo (`Rc::default()`), así que cada pasada empezaba en reposo y nunca pedía un cuadro de animación. El `hover:` **sin** `transition` no estaba afectado: ese usa el hover nativo de GPUI, y por eso la fase 27 lo vio funcionar en `examples/data.ts`.
- Arreglo (solo el host, sin protocolo): las transiciones y el set de hover viven en la `RootView` y las comparten todas las pasadas (`RenderCx` las toma prestadas en vez de recibir copias). Se agrega `stateful`, el conjunto de nodos que produjeron estado en el cuadro, y `render` poda con él al empezar el cuadro siguiente: así un nodo borrado del árbol, una fila que sale de vista o un diálogo que se cierra olvidan su estado.
- Ejemplos: en `examples/data.ts` la fila de la lista ahora usa `transition duration-300` junto a `hover:bg-accent`; en `examples/overlays.ts` el diálogo tiene un recuadro con `transition duration-500`.
- Verificado en ventana real: en la lista de 10 000 filas el fondo se funde en ~300 ms y se mantiene mientras el mouse está encima; dentro del diálogo el recuadro se funde en ~500 ms.
- `cargo test` (35 unitarios, conformidad y `no_window`) y `cargo clippy --all-targets`, limpios.
- Queda afuera: el contenedor del `Dialog` (el `div` que recibe sus clases) sigue con `hovered` fijo en falso, así que un `hover:` en la clase del nodo `Dialog` no se ve; el contenido del diálogo sí.
- Observado en los logs (INFO de GPUI, sin relación con este cambio): al activarse la accesibilidad, GPUI avisa que el elemento enfocado no tiene nodo de accesibilidad (le falta `id` y `role`). Anotado en el backlog.

### Extra — Medición: transporte, árbol grande e imágenes (2026-10-19)

Antes de decidir si el canal deja de ser JSON Lines y si las imágenes viajan como bytes, se
midió. El bench vive en [bench/](../benchmarks/) (`pwsh -File bench/run.ps1`); abajo están los números,
el método y lo que deciden. Windows, Intel Iris Xe, **host en debug** (los tiempos del host son
peores que en la build release), comparables entre sí y no absolutos.

**Transporte y aplicación del árbol** (`bench/tree.mjs`, `host --no-window`, mediana de 3 rondas;
el "apply" es el ida y vuelta de un `simulate` menos el mismo ida y vuelta con el host en reposo):

| nodos | ops | bytes | stringify ms | flush ms | apply ms |
|---|---|---|---|---|---|
| 0 | 2 | 71 | 0,0 | 0,8 | 0,5 |
| 1 000 | 2 002 | 76 863 | 0,4 | 2,9 | 24,5 |
| 10 000 | 20 002 | 787 867 | 4,9 | 24,5 | 244,7 |
| 50 000 | 100 002 | 4 027 867 | 20,8 | 106,2 | 1 269,3 |

**Piso de JSON** (`bench/payload-micro.mjs`: una línea con un valor base64, sin host):

| bytes crudos | chars base64 | bytes de línea | stringify ms | parse ms | retenido MB |
|---|---|---|---|---|---|
| 16 KB | 21 848 | 21 893 | 0,02 | 0,01 | 0,1 |
| 64 KB | 87 384 | 87 429 | 0,05 | 0,03 | 0,0 |
| 256 KB | 349 528 | 349 573 | 0,36 | 0,22 | 0,2 |
| 1 MB | 1 398 104 | 1 398 149 | 1,27 | 0,76 | 0,7 |
| 4 MB | 5 592 408 | 5 592 453 | 4,34 | 2,67 | 4,0 |

**Dibujo** (`bench/render.mjs` + `run.ps1`, dos ventanas de 5 s, dos corridas; 100 % = un núcleo):

| nodos | modo | rss MB | cpu % 1.ª ventana | cpu % 2.ª ventana |
|---|---|---|---|---|
| 200 | idle | 81,6 | 0,6–1,2 | 0,9–3,1 |
| 20 000 | idle | 338,7–339,7 | 1,9–3,1 | 1,2–1,9 |
| 50 000 | idle | 590,3–753,3 | 13,1–31,2 | 0,9–2,8 |
| 200 | spin | 84,9 | 99,1–100,9 | 98,8–100,3 |
| 20 000 | spin | 281,6–283,5 | 97,5 | 98,1–99,1 |
| 50 000 | spin | 458,3–566,1 | 98,4–100 | 97,8–99,1 |

- En reposo el host queda en **~1–3 % de un núcleo** hasta 50 000 nodos. La primera ventana de 50 000 (13–31 %) es la cola de la construcción inicial: la muestra arranca a los 5 s del commit y a esa altura todavía estaba terminando.
- Con **cualquier** animación el host queda en **~100 % de un núcleo, incluso con 200 nodos**: es el atajo ya anotado ("cada cuadro reconstruye toda la ventana"), no el tamaño del árbol.
- La memoria del árbol no virtualizado es alta: 82 MB con 200 nodos, ~340 MB con 20 000, 460–750 MB con 50 000.

**Imágenes** (`bench/payload.mjs` modo `image`, una imagen real por ruta, base 86,3 MB sin imagen):

| imagen | bytes | píxeles | rss MB | pico MB | sobrecosto de pico |
|---|---|---|---|---|---|
| 400×300 | 85 566 | 0,12 M | 87,3 | 92,6 | +6 MB |
| 2 000×1 500 | 2 098 212 | 3 M | 117,3 | 121,0 | +35 MB |
| 4 000×3 000 | 10 810 860 | 12 M | 133,9 | 133,9 | +48 MB |

**Sesgo descartado (importante):** la primera versión medía los bytes con `Input.value`, y **no
sirve**: el host maqueta ese texto como contenido de un campo, así que el número refleja el
maquetado de una cadena enorme, no el transporte (64 KB → +4 MB; 1 MB → +21 MB; 4 MB → valores no
monótonos, 98–258 MB). Esa medición se descartó y se reemplazó por el piso de JSON (arriba) más la
decodificación de una imagen real. La ruta corta (modo `path`) queda en 86,3–86,5 MB: la vía "el
cliente baja y manda una ruta" no le cuesta memoria al host.

#### Lo que deciden los números

1. **El transporte no es el cuello de botella.** 50 000 nodos son 4 MB, 21 ms de `stringify` y 106 ms de escritura al pipe; el costo está en el host aplicando nodos (~25 ms por 1 000, lineal) y, sobre todo, en el dibujo. Un mensaje `batch` ahorraría esos milisegundos de codificación, no el `apply`.
2. **Para imágenes, las tramas de blob no se justifican.** La sobrecarga de base64 de una foto de 10,8 MB es una línea de 14,4 MB, ~11 ms de codificar/parsear y ~3,6 MB de memoria: alrededor del **2 %** del total de la ruta. La decodificación cuesta **+48 MB** (12 Mpx ≈ RGBA de 48 MB) y es la misma por cualquier vía que entregue los bytes. Conclusión: (d) con base64 en una línea es aceptable y no toca el transporte; (c) sigue siendo lo más barato; el framing se descarta salvo que aparezca un caso de muchos blobs grandes y frecuentes.
3. **El árbol grande sí tiene dos costos reales:** aplicar nodos (~25 µs por nodo) y el redibujado completo por cuadro (un núcleo saturado con cualquier animación). El ítem "redibujar solo los nodos que cambiaron" pasa a ser el próximo candidato de rendimiento; el encoding del canal, no.

### Extra — Rendimiento: redibujado incremental (2026-10-20)

Paso (A) de los que quedaron planteados al cerrar la medición: "redibujar solo los nodos que
cambiaron", con el host y sin tocar el protocolo (los clientes y las transcripciones no cambian).

**Primero se midió, y la medición cambió el diagnóstico.** El host ahora informa por segundo, con
`RUST_LOG=debug`, los cuadros que dibuja una ventana y lo que cuesta su pasada propia
(`window 1: 60.0 frames/s, 0.2 ms and 14 nodes per pass`), más una línea por nodo animado. Con eso:

- Una ventana con **un solo spinner pedía cuadros a ~180 por segundo** (GPUI dibuja tan rápido como
  el bucle de eventos le permite cuando el cuadro es barato), y eso costaba **~45 % de un núcleo con
  una ventana casi vacía**. La mitad del "una animación satura un núcleo" era el ritmo, no el árbol.
- Con **200 nodos** la pasada propia costaba 1,3 ms por cuadro: el resto de esos 16 ms es dibujo y
  presentación de GPUI en esta máquina (Windows, Intel Iris Xe, host en debug). Con este árbol el
  host ya no era el costo.
- Con **20 000 nodos** la pasada propia era de **112–133 ms por cuadro** (0,7 cuadros/s): ahí sí el
  host mandaba, y **~un tercio era construir el `String` del texto de cada nodo en cada cuadro**.

**Qué se hizo (solo `host/src/render`).**

1. **Una vista por nodo que se mueve solo** (`animate-spin` en un `Icon`, `animate-pulse` en
   cualquier nodo): `AnimatedNode`, creada y descartada por commit (`sync_animated`,
   `animated_nodes`). Un cuadro reconstruye ese nodo y no lo que tiene adentro. Un `transition` **no**
   se delega: mueve unos pocos cientos de milisegundos después de un commit o de un `hover:` y una
   vista por nodo sería una por fila de lista. Tampoco se delega nada dentro de un área
   `windowControl`: ahí lo que un nodo dibuja depende del área (ver `blocks`), y la pasada de la
   ventana lo sigue dibujando.
2. **`FrameClock`**: un cuadro cada 16,7 ms como máximo, compartido por la ventana. Todos los nodos
   que pidieron cuadro se notifican juntos desde un solo temporizador (antes cada uno pedía el suyo, y
   con dos bucles el ritmo caía a la mitad). Un cuadro que tarda más que el intervalo se sigue con el
   siguiente de inmediato: no se frena un árbol pesado.
3. **El texto no se realoca por cuadro**: el contenido de cada `Text` se guarda como `SharedString`
   entre cuadros y se reemplaza solo cuando cambia (`RenderCx::text`).
4. **El `hover:` animado ya no llama `window.refresh()`**, que tiraba todas las cachés de la ventana
   (las filas de una `VirtualList` incluidas) por cambiar un color: ahora notifica la vista que dibujó
   el nodo (`RenderCx::owner`: la ventana, la vista de la lista o el cuerpo del diálogo).
5. **Se completa y corrige el atajo que estaba a medio escribir en el árbol de trabajo**: cuando no
   había transiciones en curso, `render` devolvía un `div` vacío en lugar del árbol y la ventana
   quedaba en blanco. El árbol se dibuja siempre; lo que se evita es que cada nodo animado arrastre
   su subárbol a cada cuadro.

**Números** (Windows, Intel Iris Xe, host en **debug**, 100 % = un núcleo; mismo bench y misma
máquina, antes = `HEAD`, después = este cambio):

| caso | antes | después |
|---|---|---|
| `bench/render.mjs` 200 nodos, `spin` | 102,8 % | **48,1 %** |
| `bench/render.mjs` 20 000 nodos, `spin` | 100,3 % | 100,6 % (manda el recorrido) |
| `bench/render.mjs` 20 000 nodos, `idle` | 0,6 % | 0,9–2,5 % (la cola de la construcción inicial) |
| `examples/animations.ts` en reposo (spinner + pulse) | 41,4 % | **18,8 %** |
| `examples/animations.ts` con el puntero sobre una fila (fade de 1 s) | 36,7 % | **14,8 %** |

- **Ritmo:** un spinner en una ventana casi vacía pasó de 180 a 60 cuadros/s; con dos bucles, cada uno
  y la ventana se dibujan a 60.
- **Pasada propia con 20 000 textos:** de 112–133 ms a **54 ms** por cuadro (0,7 → 1,4 cuadros/s).
- **`examples/data.ts`** (10 000 filas virtualizadas): la pasada propia es de **3–5 nodos**, el
  `hover:` de una fila anima a ~28–60 cuadros/s y la ventana vuelve a 2–4 % al terminar.
- **`examples/animations.ts`**: 14 nodos por pasada (el resto vive en las dos vistas de los bucles).
- **Release:** con la build release (la que se distribuye) `bench/render.mjs` 200 nodos + spinner queda
  en **10,9 %** de un núcleo (60 cuadros/s, pasada propia 0,17–0,20 ms) y 20 000 nodos + spinner en
  **98,4 %**: la pasada propia es de **11 ms por cuadro** (10 cuadros/s, contra 0,7 en debug).
  `examples/animations.ts` queda en **10,2 %** en reposo y **7,0 %** con el puntero sobre una fila.

**Qué queda (y por qué no se hizo).** `Window::request_animation_frame` notifica la vista que está
dibujando, y GPUI marca como sucias **también las vistas ancestras** de la que se notifica
(`Window::mark_view_dirty`), así que la ventana vuelve a recorrer el árbol en cada cuadro (54 ms con
20 000 nodos en debug, 11 ms en release): la vista propia ahorra el trabajo *debajo* del nodo, no el
recorrido. Para que una animación no recorra un árbol grande, el árbol tendría que dibujarse en una
vista que no sea ancestra del nodo animado, y eso cambia el layout (GPUI no tiene `display: contents`,
así que el árbol no se puede cortar en pedazos que se dibujen por separado). Por ahora la respuesta es
`VirtualList` —se dibuja el árbol de las filas visibles— y el límite de 60 cuadros por segundo. Sigue
en el backlog de la §8 del plan.

**Verificado en ventana real** (con el puntero real, `bench/hover-check.ps1`, que lo devuelve a su
lugar): `examples/animations.ts` —spinner y pulse a 60 cuadros/s, y el paso del puntero por las filas
de 1 s, 150 ms y 500 ms sigue fundiendo— y `examples/data.ts` —el `hover:` de una fila de la
`VirtualList` anima y la ventana vuelve al reposo. **Sin verificar a mano:** el `hover:` dentro del
cuerpo de un diálogo (en esta sesión el puntero sintético no llegó al recuadro: la ventana se abre en
el monitor secundario y el punto no cae siempre en el mismo lugar; el cuerpo ahora se notifica a sí
mismo en vez de refrescar la ventana) y el `hover:` dentro de un área `windowControl` (sigue en la
pasada de la ventana, como antes).

- `cargo test` (37 unitarios —dos nuevos de `animated_nodes`—, conformidad y `no_window`) y
  `cargo clippy --all-targets`, limpios.
- Herramientas nuevas en [bench/](../benchmarks/): `sample.ps1` (una muestra de CPU; con `-Log`, los cuadros/s
  del host) y `hover-check.ps1` (mueve el puntero real y lee los cuadros/s), documentadas en
  [bench/README.md](../benchmarks/README.md). `bench/render.mjs` acepta `--debug` y `bench/run.ps1` acepta
  `-HostPath` para medir la build release.

## §8 — Primera tanda de pasos

Acordado el 2026-10-18: se hacen de a uno, en este orden, y al terminar cada uno se hace commit en `dev` y se espera la revisión antes de seguir. Cada paso cambia el protocolo, así que agrega su sección en PROTOCOL.md y sus transcripciones de conformidad.

| # | Paso | Por qué en este lugar | Estado |
|---|---|---|---|
| 1 | **Imágenes**: nodo `Image` (archivo local, con `object-fit`) | Lo necesitan casi todas las apps y solo agrega un tipo de nodo. | hecho (fase 17) |
| 2 | **Ventanas sin marco** (frameless): sin barra de título del sistema, con zona para arrastrar y botones propios | Es una opción de `window`; conviene tenerla antes de las ventanas múltiples para diseñar las dos juntas. | hecho (fase 18) |
| 3 | **Varias ventanas**: ids de ventana, una raíz por ventana, eventos y `windowClosed` por ventana | Es el cambio más grande del modelo (hoy hay una sola ventana); los menús y la bandeja dependen de él. | hecho (fase 19) |
| 4 | **Menús**: menú contextual (clic derecho) y menú de la aplicación | Necesita saber en qué ventana se abre. | hecho (fase 20) |
| 5 | **Bandeja del sistema** (tray) | GPUI no la trae: requiere una dependencia nueva; su menú reusa el paso 4. | hecho (fase 21) |
| 6 | **Animaciones**: transiciones y clases tipo `animate-spin` | Lo menos urgente: pulido visual sobre lo que ya existe. | hecho (fase 22) |

## §9 — Segunda tanda de pasos

Acordado el 2026-10-18, al terminar la §8: las mismas reglas (de a uno, en este orden, diseño aprobado antes de programar si cambia el protocolo, commit en `dev` y revisión antes de seguir). Siguen fuera las plataformas distintas de Windows.

| # | Paso | Por qué en este lugar | Estado |
|---|---|---|---|
| 1 | **Formularios**: `Select`/`Combobox`, `Switch`, `Radio`, `Slider`, `Textarea` | Es lo que más se nota al hacer una app; todos guardan estado y comparten el patrón sincronizado/controlado de `Input` y `Checkbox`, así que salen de un solo diseño. | hecho (fase 23) |
| 2 | **Teclado y diálogos nativos**: atajos globales (también en los ítems de menú), eventos de tecla en nodos, foco, doble clic; abrir/guardar archivo, portapapeles, abrir URL, arrastrar y soltar archivos | Sin esto una app real choca rápido con un límite (el explorador de la fase 13 los necesitaba). | hecho (fases 24 y 25) |
| 3 | **Diálogos y avisos**: `Dialog` (modal), `Notification` (toast), `Tooltip` | Ventanas emergentes dentro de la app; se apoyan en el foco y el teclado del paso 2 (Escape para cerrar). | hecho (fase 26) |
| 4 | **Datos**: lista virtualizada, `Table`, `Tabs` | Resuelve las listas de miles de filas (fase 13); `Table` y la lista comparten la virtualización. | hecho (fase 27) |
### Extra — Imágenes por bytes (2026-10-20)

Punto 4 de los próximos pasos de [GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md) §8: una prop nueva en
`Image` para mandar los bytes por el mismo canal, sin agregar HTTP al host. Los números de
"Medición" ya decían que la decodificación (unos +48 MB para 12 Mpx) es igual por cualquier vía y
que base64 en una línea cuesta ~2 % del total; esta iteración solo hace falta cuando el cliente
resuelve URLs y quiere entregar la imagen ya descargada.

**Criterio de aceptación:** un `Image` con `data` (base64 estándar) dibuja los mismos formatos y
`object-fit` que `src`; reenviar `data` muestra el contenido nuevo; `data: null` o `""` vuelve a
`src`; cada nodo tiene su archivo temporal y se borra al quitar el nodo o al cerrar el árbol; base64
inválido es `invalid_props` y el host sigue.

**Protocolo (cambio):**

- `Image.data`: string con los bytes en base64 estándar (RFC 4648, con padding, sin prefijo
  `data:`). Mientras no esté vacío gana sobre `src`; `null` o `""` lo quitan y el nodo vuelve a
  `src`. Un valor que no es base64 es `invalid_props`; el mensaje de error no repite el valor
  (puede ser de megabytes). Actualizados PROTOCOL.md §5, los dos `protocol` y los cuatro clientes
  (el de txiki.js comparte el núcleo de Node).
- `data` queda en el árbol retenido, así que también cuesta memoria (~4/3 de los bytes, como el
  JSON de la medición). Para un archivo que ya está en el disco del host, `src` sigue siendo lo
  más barato.

**Host:**

- `host/src/image_data.rs`: decodifica con `base64` (dependencia nueva, 0.22) y escribe los bytes
  en un archivo por nodo dentro de una carpeta propia del árbol en el temp del sistema
  (`gpui-bridge-<pid>-<n>/<nodeId>`). La carpeta se borra en el `Drop` del árbol; el archivo de un
  nodo se borra al quitar el nodo o al limpiar `data`. Si la escritura falla (disco lleno), se
  registra en stderr y el nodo queda vacío: no es un error de protocolo, como un archivo que falta.
- El render usa el archivo cuando `data` no está vacío y, si no, `src`. GPUI adivina el formato por
  los bytes (no por la extensión), así que se conservan GIF/WebP animados, SVG y los demás formatos
  que ya tenía la fase 17. Cada `data` recibido empuja el archivo a `image_reloads`, que ahora es
  `Vec<PathBuf>` junto con los `src`, para que GPUI no muestre la versión anterior del caché.
- La advertencia de `src` inexistente se calla cuando el nodo trae `data` (ese `src` no se usa).

**Clientes y ejemplo:**

- Node/txiki.js y React: `data?: string` junto a `src` (en React `src` pasó a opcional). Python:
  documentado en `app.image(**props)`. Los clientes no codifican nada: el que descarga la imagen
  hace `Buffer.from(bytes).toString("base64")` o `base64.b64encode(bytes)`.
- `examples/image-bytes.ts`: la misma imagen dos veces, por ruta y por bytes, y un botón que
  reenvía los bytes.

**Pruebas:**

- `cargo test`: 40 unitarios (nuevos: escritura por nodo, reenvío, `null`, borrado con el nodo y con
  el árbol, base64 inválido y tipo inválido), las transcripciones de conformidad (casos nuevos en
  `conformance/props.txt`) y `no_window`. `cargo clippy --all-targets`, limpio.
- `client-node`: 15 tests; `client-react`: 13; `client-python`: 15. Los tres incluyen un `Image` con
  `data` y el error de base64 inválido (`Prop 'data' of Image must be standard base64`).

**Verificación en ventana real (2026-10-20):** se abrió `examples/image-bytes.ts`; el host creó la
ventana "Image from bytes" y, con el proceso vivo, la carpeta temporal tenía el archivo del nodo con
los **274 bytes del PNG de origen** (hash idéntico) y sin errores de carga en stderr. La captura de
pantalla no sirvió para confirmar los píxeles en esta sesión (la ventana quedó fuera del área
capturada), así que **queda pendiente la confirmación visual** de que las dos copias se ven; el
camino de dibujo es el mismo `img(PathBuf)` que la fase 17 ya verificó.

### Corrección — Desbordamiento de pila del host en debug (2026-10-20)

`node examples/frameless.ts` terminaba con `gpui-bridge host exited (code 3221225725)` y
`thread 'main' has overflowed its stack` justo después de crear el dispositivo Direct3D.
`0xC00000FD` es `STATUS_STACK_OVERFLOW`: no es un fallo de DirectX ni de `frameless` (los
avisos `0x887A002D` y «Failed to get DXGI debug interface» son normales en esta máquina).

**Causa.** El recorrido del árbol (`RenderCx::node_raw`) recursa una vez por nivel de anidación
y, en un build **sin optimizar**, Rust reserva *toda* la pila de la función al entrar, antes de
saber qué rama se usa. `node_raw` era una sola función con las 17 ramas (todos los tipos de
nodo, con sus elementos y closures), así que cada nivel costaba **~197 KiB** de pila (medido:
nivel 0 = 72 KiB, +197 KiB por nivel). Con los 1 MB que Windows da al hilo principal, el árbol
de `frameless.ts` (4 niveles + el `Icon` del botón de cerrar) moría al construir el quinto
nivel. Un árbol de solo `div`s anidados también caía, a partir de 5 niveles.

**Qué se hizo.**

- `host/src/render`: cada tipo de nodo tiene su función (`div_node`, `icon_node`, `table_node`…);
  `node_raw` solo mira `class` y despacha. Cada nivel paga el marco de su tipo (~27 KiB, 7 veces
  menos) y no el de los 17.
- `host/build.rs` (nuevo): el binario pide **8 MB** de pila para el hilo principal
  (`/STACK:8388608` con MSVC, `-Wl,--stack` con GNU), la misma que Rust ya da en Linux. Solo
  afecta al enlazado del binario; las dependencias no se recompilan.

**Pruebas.**

- Reproducido y verificado con un cliente mínimo que habla el protocolo y mantiene la entrada
  abierta: antes, una cadena de 5 `div`s ya desbordaba; ahora 20 niveles funcionan sin
  `build.rs` y **100** con él (300 todavía no: ~27 KiB por nivel).
- `cargo test` (40 unitarios + conformidad + `no_window`) y `cargo clippy --all-targets`, limpios.
- Los 14 ejemplos de `examples/*.ts` abren su ventana y siguen vivos a los 4 s; `frameless.ts`
  incluido (antes moría antes del primer frame), con el host en debug y en release.

## Accesibilidad y componentes de gpui-kit (2026-10-20)

Pedido del usuario: dar soporte a la mayor cantidad de componentes de **GPUI Kit** de las
categorías **Basic, Form, Layout y Advanced**, dejando fuera los que necesiten una dependencia
extra que engrose el host; y agregar **accesibilidad** como dice la documentación.

**Accesibilidad (PROTOCOL.md §16).** GPUI informa a la tecnología asistiva (AccessKit) solo los
elementos que tienen **id y rol**; el marco avisaba que «el elemento enfocado no tiene nodo de
accesibilidad» porque la raíz de la ventana no tenía ninguno. Ahora:

- Todo nodo acepta un prop `a11y`: `id`, `role` (nombres de ARIA), `label`, `description`,
  `keyshortcuts`, `checked` (`true`, `false` o `"mixed"`), `selected`, `expanded`, `disabled`,
  `value`, `min`, `max`, `step`, `level` y `orientation`.
- Se valida como cualquier otro prop (`invalid_props`): una clave desconocida, un rol que no
  existe o un tipo equivocado se rechazan en vez de quedar en silencio; `null` lo quita.
- Se aplica al elemento del `div` y del `text`; para un componente (`Button`, `Slider`…) se
  envuelve en una caja que lleva el nodo de accesibilidad, y solo cuando el prop está presente.
- La raíz de cada ventana se informa como `window` con el título de la ventana, que es lo que
  faltaba para que el foco propio de la ventana tuviera nodo.
- `host/src/a11y.rs` (nuevo) tiene el análisis, la validación, la tabla de roles y la
  aplicación; `schema.rs` la engancha en `PropKind::Accessibility`.

**Componentes nuevos (PROTOCOL.md §17), 23 tipos de nodo (40 en total).**

| Categoría | Nodos | Estado |
|---|---|---|
| Basic | `Alert`, `Badge`, `Tag`, `Kbd`, `Progress`, `Spinner`, `Skeleton`, `Rating`, `Pagination`, `Collapsible`, `Accordion`, `Avatar`, `Stepper` | hechos |
| Form | `NumberInput`, `OtpInput`, `Form`, `Field` | hechos (`Input`, `Textarea`, `Select`, `RadioGroup`, `Checkbox`, `Switch`, `Slider` ya existían) |
| Layout | `GroupBox`, `DescriptionList`, `StatusBar`, `Toolbar`, `Resizable` | hechos (`Dialog` y las notificaciones ya existían) |
| Advanced | `Calendar` | hecho (`Table`, `Tabs`, `VirtualList`, los menús ya existían) |

- Los que tienen estado del usuario lo guardan como los demás controles: modo **sincronizado**
  (el host lo cambia y avisa `change`) o **controlado** (`controlled: true`, solo avisa). Son
  `Rating` (`value`), `Pagination` (`page`), `Accordion` (el `open` de cada `items[i]`),
  `Stepper` y `Calendar` (`value`), más `NumberInput`/`OtpInput` como un `Input`.
- `Accordion` recibe sus secciones como `items` (`{id, title, content, open, disabled}`) porque
  el árbol no tiene un nodo por sección: el contenido es **texto**, documentado como límite (una
  sección con contenido rico se arma con un `Collapsible` con hijos).
- `Resizable` recibe un hijo por panel y `sizes` con el tamaño inicial en píxeles de cada uno:
  el panel es un contenedor de gpui-component y el modelo no tiene props por hijo.
  `StatusBar` usa `left`/`right` (texto), que es lo que expone el componente.
- `NumberInput` reutiliza el estado de los `Input` (el mismo `InputState`); `OtpInput` tiene su
  propio `OtpState` por nodo; `Calendar` un `CalendarState` por nodo y se enfoca con `focus`.

**Quedaron fuera a propósito.** `Editor` (resaltado con `tree-sitter`, más las gramáticas) y
`Speech` (dictado con el reconocedor del sistema): son features opcionales de gpui-kit que
agregan mucho tamaño al host, que es justo el criterio pedido. `Dock`, `Command`, `Settings`,
`Tree`, `List`, `Carousel`, `Diff`, `Chart` y `Sidebar` necesitan delegates o máquinas de estado
propias y no están mapeados todavía (quedan anotados en la §8 del plan). `TextView`, `Bubble`,
`Message`, `MessageScroller`, `Marker` y `Questionnaire` tampoco están: sus props son
composiciones con elementos (markdown/HTML, burbujas con avatar, preguntas de varios pasos) que
el árbol todavía no sabe expresar.


**Clientes.** `client-node/src/protocol.ts`: los 23 tipos, sus props, sus eventos y el tipo
`A11y` (más `SharedProps`, que agrega `a11y` a **todos** los tipos de nodo). `app.ts`:
un método por componente (`app.alert(...)`, `app.calendar(...)`…). `client-python`: los mismos
métodos. Los clientes no codifican nada más: siguen mandando `create` y `setProps`.

**Ejemplo.** `examples/components.ts`: una ventana con alerta, badges, tags, teclas, progreso,
spinner, rating, paginación, pasos, acordeón, formulario con `Field`s, lista de descripción,
caja con título, calendario, paneles redimensionables, barra de herramientas y barra de estado,
todo con `a11y` donde el control no se nombra solo.

**Pruebas.**

- `cargo test`: 48 unitarios (nuevos: `a11y` — lectura de todos los campos, rechazo de claves y
  tipos, tabla de roles, valor numérico solo con rango—, `schema` —ida y vuelta de nombres,
  `a11y` en todos los tipos—), conformidad y `no_window`. `cargo clippy --all-targets`, limpio.
- Transcripciones nuevas: `conformance/a11y.txt` (a11y en `div`, `text`, `Button`, `Slider` y
  `Checkbox`, errores y `null`) y `conformance/basic.txt` + `conformance/layout.txt` (los 24
  tipos, sus `change`/`close`/`submit` simulados, valores fuera de rango, pasos y secciones
  deshabilitadas, y props mal formados).
- `client-node`: 17 tests; `client-python`: 17. Los nuevos crean todos los componentes contra el
  host real sin ventana, despachan sus eventos y comprueban que un `a11y` con una clave mal
  escrita da `invalid_props`.

**Verificado en ventana real (2026-10-20).** `node examples/components.ts` abrió la ventana
"Components" y siguió viva 10 s, con 35 nodos por pasada y ~55–60 cuadros/s en debug (el spinner
es el único nodo animado). En stderr no hubo avisos de props desconocidas ni el aviso de
accesibilidad del elemento enfocado; los únicos mensajes son los de DirectX de esta máquina
(`0x887A002D` y el depurador DXGI), los mismos de siempre. **Sin verificar a mano:** el aspecto
visual de cada componente y lo que anuncia un lector de pantalla real (no hay uno instalado en
esta sesión); el camino de dibujo y la ausencia de avisos es lo comprobado.

### Documentado — Análisis de los componentes restantes (2026-10-20)

Se revisaron las APIs reales de gpui-component 0.7.3 para los nueve componentes que quedaron
fuera y se dejó el análisis en [GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md):

- **Corrección de una afirmación previa:** no es que "no se puedan" agregar, es que no encajan en
  el modelo *un nodo = props planas + hijos*. Solo `Editor` y `Speech` quedan fuera por tamaño
  (features con `tree-sitter` y `cpal`); los otros nueve no agregan dependencias.
- **Clasificación por coste real:** `Tree`, `Diff`, `Carousel` y `Sidebar` son datos + un slot
  (S–M); `Chart`, `List`, `Command` y `Settings` necesitan datos anidados, un delegate o un
  mini-lenguaje de campos (M–L); `Dock` rompe el modelo (paneles con trait, layout persistente y
  drag&drop) y necesita un ADR.
- **Impacto detallado** en el host (slots, validadores recursivos, caché por contenido, eventos
  con payload objeto, pila y rendimiento), en el protocolo (props anidadas, `data` opcional,
  operación nueva solo para `Dock`) y en los clientes (tipos recursivos; en React los datos van
  en props, y `Dock` necesita una convención).
- **Plan por tandas** con criterios de aceptación: 1) `Tree`+`Diff`+`Carousel`; 2) `Sidebar`+
  `Chart`; 3) `Command`+`Settings`; 4) `List`; 5) `Dock` (solo con ADR).

No se tocó código en este paso: la lista de tipos de nodo sigue en 40 (PROTOCOL.md §5). El
documento quedó enlazado desde la §8 del plan y desde el README.

### Extra — Estabilización del repositorio (2026-10-21)

El repositorio pasó a tener `origin` (<https://github.com/InDIOS/gpui-bridge>). Antes de elegir el
siguiente hito se cerró lo que faltaba para que un checkout y CI sean reproducibles y para que los
paquetes puedan publicarse sin arreglos manuales. No cambia el protocolo ni el host.

**Verificación de partida** (todo en verde en esta máquina, Windows x64, Node 24.22, pnpm 12.4.2,
Python 3.12, Rust 1.99.1):

- `cargo test --workspace`: 48 unitarios, la transcripción de conformidad y `no_window`.
- `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all -- --check`, limpios.
- `pnpm test:node` 17/17, `pnpm test:react` 14/14, `pnpm test:python` 17/17.
- `pnpm --filter gpui-bridge-examples typecheck`, limpio.
- `gpui-bridge-host --version` → `0.1.0 (windows-x86_64, protocol 1)`, en debug y en release.

**Cambios**

- **Metadatos de publicación:** `license`, `author`, `keywords`, `homepage`, `repository` (con
  `directory`) y `bugs` en los dos `package.json`; `authors`, `keywords`, `[project.urls]` y más
  classifiers en `pyproject.toml`. `gpui-bridge-react` declara `os: ["win32"]` y `cpu: ["x64"]`
  como el cliente de Node, para fallar al instalar en vez de al abrir una ventana.
- **Enlaces:** `OWNER` reemplazado por `InDIOS` en los README de los paquetes; `RELEASING.md` ya
  no pide ese reemplazo y explica dónde vive el nombre del repositorio.
- **`.gitattributes` nuevo:** `* text=auto eol=lf` (el índice ya estaba en LF, pero 51 archivos
  llegaban al working tree en CRLF en Windows) y binarios declarados (`*.png`, `*.exe`, `*.whl`,
  `*.tgz`), para que un checkout en macOS/Linux produzca los mismos bytes y las transcripciones de
  `conformance/` se comparen igual.
- **Rust fijado:** `rust-toolchain.toml` pasa de `stable` a `1.99.1`, la versión con la que se
  verificó; CI usa la misma (`RUST_VERSION` en `ci.yml`). Se suben juntas.
- **CI:** `cargo fmt --all -- --check` (el `rust-toolchain.toml` decía que CI lo corría y no era
  cierto), `clippy ... -- -D warnings`, `--locked` en clippy/test/build, `timeout-minutes` por job
  y `concurrency` para cancelar runs viejos de la misma rama.
- **`CONTRIBUTING.md`:** los checks previos a un commit son los que corre CI, más el type-check de
  los ejemplos.
- **Plantillas de GitHub:** `.github/PULL_REQUEST_TEMPLATE.md` repite las reglas de
  `CONTRIBUTING.md` como checklist (protocolo, fmt, clippy, tests, empaquetado, historial) y
  `.github/ISSUE_TEMPLATE/bug_report.md` pide `bin/host.json`, la versión del cliente y los logs
  de stderr.
- **Plan §8:** se agregó la hoja de ruta propuesta (M0–M6) con criterios de aceptación e impacto en
  el protocolo; los puntos del backlog anterior quedan como catálogo en §8.2.

**Verificación del empaquetado** (fuera del repositorio, sin `GPUI_BRIDGE_HOST`):

- `pnpm pack:node` → `gpui-bridge-0.1.0.tgz` (7.8 MB) con `bin/gpui-bridge-host.exe` y
  `bin/host.json` (`0.1.0`, protocolo 1, `win32-x64`). Instalado con npm en una carpeta temporal:
  `createApp({ headless: true })`, dos `commit` y `close()` → handshake y salida 0.
- `pnpm wheel` → `gpui_bridge-0.1.0-py3-none-win_amd64.whl` (7.8 MB), instalado en un venv
  temporal: `App(headless=True)`, dos `commit` y `close()` → salida 0.
- La primera pasada del wheel falló y eso evitó un bug publicado: el classifier
  `License :: OSI Approved :: MIT License` choca con `license = "MIT"` + `license-files` de
  PEP 639 (setuptools ≥ 77 lo rechaza). Se quitó el classifier; la expresión de licencia queda
  como única fuente.

**Pendiente fuera del repositorio** (no se puede hacer desde un commit): confirmar el primer run
verde de CI en `dev` y `main`, proteger `main` exigiendo CI y revisión, y crear los secretos
`NPM_TOKEN` y `TWINE_*` solo cuando se decida publicar (M5).

- **CI en verde (2026-10-21).** El primer run falló solo en `pack`: PowerShell no expande globs
  para comandos nativos, así que `tar.exe` recibía `packages/client-node/gpui-bridge-*.tgz` literal.
  El step ahora lista el tarball con `Get-ChildItem`, falla si falta `bin/gpui-bridge-host.exe` o
  `bin/host.json` y comprueba lo mismo en el wheel. Tras el arreglo, `dev` pasó en `host`, `clients`
  y `pack`. Proteger `main` sigue pendiente, para cuando el repositorio sea público.

### Fase 28 — Ventanas: posición, tamaño mínimo, minimizar/restaurar y eventos (2026-10-21)

Primer hito de la hoja de ruta M0–M6 ([GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md) §8.1). No sube
`PROTOCOL_VERSION`: son campos **opcionales** de `window` y un mensaje nuevo del host, así que un
host viejo ignora los campos y un cliente viejo nunca los pide.

**Criterios de aceptación:** posición inicial y tamaño mínimo aplicados al abrir; minimizar y
restaurar sin cerrar; eventos de foco, redimensionado, movimiento y visibilidad, opt-in;
`PROTOCOL.md` §3 y §6, transcripción propia, ejemplo y prueba en ventana real.

**Estado:** completada la parte automática; **falta la prueba en ventana real** (ver abajo).

- **API de GPUI verificada en la fuente instalada** (`gpui-pre-0.3.9`, regla 2):
  `WindowOptions::window_bounds` con `WindowBounds::Windowed(Bounds)`, `WindowOptions::window_min_size`,
  `Window::minimize_window()`, `Window::activate_window()` (restaura si está minimizada),
  `Context::observe_window_bounds/activation/visibility`. **No hay `hide()` por ventana**: `App::hide`
  oculta la aplicación y `hide_window` no existe, así que "ocultar sin cerrar" se resuelve con
  `minimized`, y restaurar con `activate_window`. Queda dicho en PROTOCOL.md §3.
- **Protocolo (§3):** `window` acepta `x`, `y` (posición inicial, píxeles lógicos; se leen al abrir,
  como `frameless`), `minWidth`/`minHeight` (tamaño mínimo; también al abrir), `minimized` (acción:
  `true` minimiza, `false` restaura y trae al frente; no se recuerda entre commits) y `events`
  (lista que **reemplaza** la anterior; `[]` desuscribe). Nombres: `focus`, `blur`, `resize`, `move`,
  `visibility`; uno desconocido es `invalid_json`.
- **Protocolo (§6):** mensaje `{"type":"windowEvent","id":…,"event":…}`. `move` y `resize` llevan
  `x`,`y`,`width`,`height`; `visibility` lleva `visible`. Solo se envían los suscritos, y solo
  cambios: la ventana que abre reporta `focus` si arranca activa, pero no reporta su tamaño ni su
  posición iniciales (son los que pidió el cliente). `resize` cuando cambió el tamaño; `move`
  cuando solo cambió la posición.
- **Host:** `WindowSpec` gana `x`, `y`, `min_width`, `min_height` y `events`; las peticiones de
  `minimized` van en una cola aparte (`take_window_requests`) que `sync_windows` aplica **después**
  de abrir o actualizar, así que `{"minimized":true}` funciona en el mismo commit que abre la
  ventana. `RootView` registra tres observadores (`activation`, `bounds`, `visibility`), los primea
  con la primera llamada para no mandar ruido y filtra con `tree.wants_window_event`. Los
  observadores se `detach()`: viven lo que vive la vista, que vive lo que vive la ventana.
- **Clientes:** Node `WindowOptions` con los campos nuevos, `AppWindow.on/off` para los cinco
  eventos (manda la lista de `events` solo cuando cambia el conjunto de suscripciones) y
  `App.on("windowEvent")` para la ventana principal o para todas; Python `Window.on/off` (con
  `ValueError` para un nombre desconocido) y `App.on_window_event`; React `<Window onFocus onBlur
  onResize onMove onVisibility>` (suscripción por prop, se da de baja al quitarla) y los campos
  nuevos en `WindowOptions`, que la ventana principal recibe por `render(element, { window })`.
- **Ejemplo:** `examples/windows.ts` abre en 80,80 con mínimo 320x220, muestra el tamaño en un
  texto que actualiza `resize` y tiene un botón "Minimize for 1 s" que minimiza y restaura.
- **Verificación automática:** 49 unitarios del host (nuevo: eventos y peticiones de `minimized`;
  el test de `window` cubre la lectura de los campos nuevos), la transcripción nueva
  `conformance/window-events.txt` (campos válidos, nombre de evento desconocido, tipos erróneos,
  lista vacía), las 19 transcripciones restantes, `no_window`, 18 tests de Node, 15 de React y 18
  de Python; `clippy -D warnings`, `fmt` y el type-check de los ejemplos, limpios.
- **Prueba en ventana real (2026-10-21).** Se condujo el host con un script (una ventana "probe"
  a 80,80, mínimo 300x200, minimizar y restaurar) y destapó tres cosas que los tests no podían ver:
  - **`pnpm dev` ejecutaba un host viejo.** `scripts/dev.mjs` prefería `target/release`, que era
    anterior a M1 (empaquetado de M0); el debug era el actual. Por eso "no funcionaba nada" en
    `pnpm dev windows`. Ahora prefiere `debug` (el orden que ya usan los clientes) y `--debug`
    imprime la ruta del host en uso, porque un binario viejo y una función rota se ven igual.
  - **La geometría se leía del área cliente.** `Window::bounds()` devuelve el origen del área de
    cliente (80,124 con `x`/`y` = 80,80: +44 px de barra de título), así que un `move` tras
    minimizar y restaurar "desplazaba" la ventana y un cliente que guardara ese valor y lo
    devolviera a `x`/`y` se iría bajando en cada arranque. Se usa
    `window.window_bounds().get_bounds()`: origen del marco (lo mismo que fija `x`/`y`) y tamaño
    cliente (lo mismo que `width`/`height`).
  - **`visibility: false` no llegaba al minimizar.** El observador de visibilidad no llama al
    registrar (a diferencia de activación y bounds), así que el primer `Hidden` se consumía como
    "prime" y se perdía. Ahora el estado se primea con `window.visibility()` al registrar.
  - Al minimizar, Windows mueve la ventana a `-32000`; con `window_bounds` ese cambio no aparece
    como `move`, así que no hace falta filtrarlo.
  - Secuencia verificada tras el arreglo: `focus` al abrir; `visibility false` + `blur` al
    minimizar; `focus` + `visibility true` al restaurar; sin `move` espurios.
- **Pendiente (regla 7):** la comprobación visual sigue en manos del usuario: que la ventana se
  vea en 80,80, que no baje de 320x220, que el texto de geometría siga al redimensionado y que el
  botón minimice y restaure. Lo automático cubre el protocolo y los eventos; el dibujo, el mouse y
  el tamaño mínimo en pantalla no.
- **Prueba visual del usuario (2026-10-21).** `pnpm dev windows` pasó los tres puntos: la ventana
  abre en 80,80, no baja de 320x220, el texto de geometría sigue al redimensionado y el botón
  minimiza y restaura a 1 s. Con eso M0 y M1 quedan cerrados en la hoja de ruta.
- **Fuera de este paso:** mover o redimensionar la ventana desde el cliente después de abrirla
  (`x`/`y`/`minWidth`/`minHeight` se leen solo al abrir, y avisan por stderr si cambian), ocultar
  de verdad (GPUI no lo expone), estado maximizado/pantalla completa en el protocolo, y eventos de
  DPI o de pantalla.

### Fase 29 — M2: Tree, Diff y Carousel (2026-10-21)

Segundo hito de la hoja de ruta ([GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md) §8.1), tanda 1 de
[GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md) §9. No sube `PROTOCOL_VERSION`: son props
nuevas y objetos dentro de `value`, que un host viejo ignora (PROTOCOL.md §6) y un cliente viejo
no envía.

**Criterios de aceptación:** props validadas (árbol y patch incluidos), selección y expansión
reaplicadas al reconstruir, transcripción propia, ejemplo y ventana real; clientes Node, React y
Python en el mismo cambio.

**Estado:** completada la parte automática; **falta la prueba en ventana real** (ver abajo).

- **API de gpui-component 0.7.3 verificada en la fuente instalada** (regla 2): `tree::Tree` con
  `TreeState::new/items/set_items/set_selected_item/selected_item/scroll_handle` y
  `TreeEvent::{Expanded, Collapsed}`; `diff::{Diff, DiffState, DiffFile::parse, DiffEvent,
  DiffMode, DiffLineRange}`; `carousel::{Carousel, CarouselContent, CarouselItem,
  CarouselPrevious, CarouselNext, CarouselPagination, CarouselPaginationItem, CarouselState,
  CarouselEvent}`. Dos detalles que fijaron el diseño: `TreeState::set_items` **resetea** la
  selección, y al revelar un item seleccionado dentro de una carpeta cerrada
  `set_selected_item` **emite** `TreeEvent::Expanded` por cada ancestro; `CarouselState` emite
  `Change` solo desde los caminos de usuario (`select_*`) y sus setters son silenciosos.
- **Protocolo (§5, §6 y §18):** `Tree` (`items` recursivo, `value`, `controlled`; eventos
  `change`, `expand`, `collapse`), `Diff` (`patch`, `mode`, `lineNumbers`, `softWrap`, `header`;
  `change` con `{path, side, start, end, endSide}` o `null`) y `Carousel` (`value`, `axis`,
  `looping`, `controls`, `pagination`, `controlled`; un hijo por diapositiva; `change` con el
  índice). `value` admite ya un objeto para `Diff`, como `Table` con `sort`.
- **Host — Tree:** un `TreeState` por nodo y `TreeSlot` con la lista normalizada
  (`ItemSpec`, comparable) y un mapa de expansión elegida por el usuario. El estado se reconstruye
  **solo** cuando `items` cambia (comparar el prop, como `Select` con `options`); tras
  `set_items` se reaplica `value`. La expansión del usuario sobrevive al rebuild para los ids que
  siguen existiendo y cuyo `expanded` no cambió el cliente. Un `Cell` de silencio evita que la
  expansión automática de ancestros (al revelar la selección) se confunda con un clic. La
  selección se reporta desde el `on_click` de la fila (`change` con el id); los `ListItem` llevan
  sangría por profundidad y chevron con los iconos incluidos, y las filas se virtualizan solas.
- **Host — Diff:** `check_patch` parsea el patch al validar la prop, así que uno mal formado es
  `invalid_props` y nunca llega a `DiffState`; el estado se reconstruye solo si cambian `patch` o
  `mode` (`set_mode` conserva la selección cuando solo cambia la presentación). `change` se emite
  desde `DiffEvent::SelectionChanged`, con `null` al limpiar.
- **Host — Carousel:** un `CarouselState` por nodo; el número de diapositivas es
  `node.children.len()`, y `sync` aplica recuento, eje, bucle y el índice de `value`
  (`set_selected_index` lo acota). Los hijos se envuelven en `CarouselItem` en orden; los
  controles y los puntos son opcionales.
- **Simulación (`--no-window`):** `change`/`expand`/`collapse` en `Tree` (id desconocido →
  `invalid_operation`, hoja o item deshabilitado → nada), `change` en `Diff` (`null` o rango con
  ruta del patch; ruta ausente → `invalid_operation`, objeto mal formado → `invalid_json`) y
  `change` en `Carousel` (índice fuera de rango → `invalid_json`). Transcripción nueva
  `conformance/advanced.txt` (props válidas, selección, expansión, controlado, los errores de
  `simulate` y diez `invalid_props`).
- **Clientes:** Node añade `app.tree/diff/carousel`, los tipos `TreeNode`, `DiffSelection`,
  `TreeProps`, `DiffProps`, `CarouselProps` y los eventos tipados. Para liberar el nombre `tree`
  el árbol de operaciones interno de `App` pasó a llamarse `wire` (solo uso interno). React añade
  `<Tree>`, `<Diff>` y `<Carousel>`, `onExpand`/`onCollapse` y deduce `controlled` de `value` como
  el resto de controles. Python añade `tree`, `diff` y `carousel`.
- **Ejemplo:** `examples/advanced.ts` (árbol de un repositorio con selección y expansión, patch
  unificado con selección de líneas y carrusel con puntos), ejecutable con
  `pnpm dev advanced`.
- **Verificación automática:** 52 unitarios del host (nuevos: validación recursiva del árbol,
  reaplicación de la expansión y parseo del patch), las 21 transcripciones de `conformance/`,
  `no_window`, 19 tests de Node, 16 de React y 19 de Python; `clippy -D warnings`, `fmt` y el
  type-check de los ejemplos, limpios.
- **Corrección tras la primera prueba en ventana real (2026-10-21).** `pnpm dev advanced` mostró
  los tres contenedores vacíos (el carrusel solo con los puntos) y destapó dos fallos de layout
  que ningún test sin ventana podía ver:
  - **`a11y::wrap` encogía los componentes a su ancho de contenido.** Es un `div().flex()` extra;
    dentro de una columna, un hijo `size_full()` no tiene un ancho definido contra el que
    resolverse, así que `Tree` y `Diff` quedaban con un ancho mínimo. Los tres componentes de datos
    aplican ahora el `a11y` en **su propia** caja con tamaño (`component_a11y`), sin envoltorio
    extra; el resto de componentes conserva el envoltorio de siempre.
  - **El carrusel no tenía altura definida.** El estado contaba las diapositivas (los puntos
    salían), pero `CarouselContent` se dimensionaba por contenido y una diapositiva `size-full`
    resolvía a 0. Ahora la raíz del carrusel es `h_full()` y el contenido `flex_1().min_h_0()`, de
    modo que la diapositiva llena el espacio que dejan los controles.
- **Prueba en ventana real (2026-10-21).** Con el host en debug, `pnpm dev advanced` y clics
  guionizados sobre la ventana (más capturas de la propia ventana con `PrintWindow`), se
  comprobó lo que ningún test sin ventana cubre:
  - **Tree:** muestra la jerarquía (`src` abierto con `main.rs` y `ui`, y `README.md` y `target`
    deshabilitado atenuado); un clic en `README.md` mueve la selección y muestra
    `Tree: selected README.md`; un clic en `ui` lo expande y aparece `tree.rs`.
  - **Diff:** pinta el patch con cabecera, hunk y líneas tintadas; un clic en el número de la
    línea 2 la selecciona (azul) y muestra `Diff: notes.txt original 2-2`.
  - **Carousel:** la diapositiva se ve y llena la caja; las flechas `‹ ›` y los tres puntos están
    visibles; un clic en `›` pasa a la diapositiva 2, mueve el punto activo y muestra
    `Carousel: slide 2`.
- **Dos fallos más que destapó esa prueba.**
  - **`ListItem` apila sus hijos directos en un bloque**, así que el chevron y el texto del árbol
    salían en dos líneas; ahora van dentro de un `h_flex` (una fila).
  - **`CarouselPrevious`/`CarouselNext` se posicionan en absoluto sobre el contenido y en este
    host quedaban fuera de la caja visible.** Se sustituyen por dos botones propios (iconos
    `chevron-left`/`chevron-right`) en una fila bajo la diapositiva, que llaman a
    `select_previous`/`select_next` (el mismo camino que emite `change`).
- **Pendiente (regla 7):** nada visual de esta tanda; el teclado del carrusel y del árbol se
  ejercitó por los botones y el foco, y lo automático cubre el resto.
- **Fuera de este paso:** `value` como selección de líneas de `Diff` (la selección es del
  componente), búsqueda o filtrado en `Tree`, carga incremental, y `Dock`/`Command`/`Settings`/
  `List`/`Chart`/`Sidebar`, que siguen en las tandas siguientes.

### Fase 30 — M3: Sidebar y Chart (2026-10-21)

Tercer hito de la hoja de ruta ([GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md) §8.1), tanda 2 de
[GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md) §9. No sube `PROTOCOL_VERSION`: son dos
tipos de nodo y props nuevas que un cliente viejo no envía.

**Criterios de aceptación:** los de M2 (props validadas, transcripción, ejemplo, ventana real y
los tres clientes en el mismo cambio) más una prueba con series de 1000 puntos y un árbol
profundo.

**Estado:** hecha, con la prueba en ventana real.

- **Protocolo (§5 y §19):** `Sidebar` (`items` en secciones `{label?, items}` de entradas
  `{id, label, icon?, disabled?, open?, children?}` a cualquier profundidad, `value`,
  `collapsed`, `collapsible` `icon`/`offcanvas`/`none`, `side`, `controlled`; `change` con el id)
  y `Chart` (`chart` `line`/`bar`/`pie`, `data` `[{label, value}]`, `color` con los nombres del
  tema, `name`, y las opciones de cada gráfico; sin eventos). Ninguno toma hijos.
- **Host — Sidebar:** `check_items` valida secciones, entradas, ids únicos en todo el sidebar e
  iconos incluidos; `build` arma `Sidebar`/`SidebarGroup`/`SidebarMenu` de gpui-component con el
  activo marcado por `value`. Un clic en una entrada deshabilitada o en la activa no envía nada;
  una entrada padre reporta su id y abre o cierra su submenú.
- **Host — Chart:** `ChartSlot` guarda los puntos parseados y solo se relee cuando cambia `data`
  (una serie de 1000 puntos no se parsea en cada cuadro). Línea (`curve`, `dot`, `yMin`/`yMax`),
  barras (`horizontal`) y tarta (`innerRadius`, `padAngle`, colores `chart-1`…`chart-5` por
  porción).
- **Simulación y conformidad:** `change` en `Sidebar` (id desconocido → `invalid_operation`,
  deshabilitada o activa → nada; `Chart` no tiene eventos). Transcripción nueva
  `conformance/charts.txt` (props válidas, selección, submenú, controlado y diez
  `invalid_props`).
- **Clientes:** Node `app.sidebar/chart` con `SidebarSection`, `ChartDatum` y los tipos de props;
  React `<Sidebar onChange>` y `<Chart>`; Python `sidebar` y `chart`.
- **Ejemplo:** `examples/dashboard.ts` (`pnpm dev dashboard`): sidebar con submenú y entrada
  deshabilitada, línea de 1000 puntos con botón para reenviarlos, barras, tarta y un árbol de 10
  niveles (2047 items).
- **Tres fallos que destapó la prueba en ventana real** y que ningún test sin ventana ve:
  - **Las 1000 etiquetas del eje x** se dibujaban todas (gpui-component etiqueta cada punto por
    defecto) y formaban una barra negra. El host pasa ahora `x_tick_count`/`band_tick_count`:
    12 etiquetas por defecto, repartidas del primer punto al último, y el prop `xTicks` (entero
    desde 0) deja al cliente elegir el número (0 ninguna; uno mayor que los puntos, todas).
    `yTicks` hace lo mismo con las marcas del eje de valores (por defecto 5, extremos incluidos;
    gpui-component sube a 2 lo que quede por debajo).
  - **`padAngle` está en radianes** (`Pie` de gpui-base trabaja sobre `TAU`). El ejemplo y la
    transcripción usaban `2` (≈115°), que se tragaba toda porción menor: solo se veía la de
    Rust, aunque el hover sí encontraba las otras. Se documenta la unidad y el ejemplo pasa a
    `0.03`.
  - **El tooltip de la tarta no decía qué porción era** («21 (21.0%)»); ahora usa
    `tooltip_name` con la etiqueta («TypeScript 21 (21.0%)»).
- **Prueba en ventana real (2026-10-21).** Host en debug, `pnpm dev dashboard`, clics guionizados
  y capturas de la ventana: clic en Inbox, Settings (abre el submenú con Palette atenuada) y
  Theme mueven el activo y muestran `Sidebar: <id>`; Palette deshabilitada no cambia nada;
  «Toggle sidebar» lo pliega a iconos; «Refresh the series» reenvía los 1000 puntos (0,6 ms en
  el cliente) y la línea se redibuja; el árbol de 10 niveles se ve con tres niveles abiertos y un
  clic expande `b`. Sin pánicos en el stderr del host.
- **Verificación automática:** 58 unitarios del host, las 22 transcripciones de `conformance/`,
  `no_window`, 21 tests de Node, 17 de React y 21 de Python; `clippy -D warnings`, `fmt` y el
  type-check de los ejemplos, limpios.
- **Fuera de este paso:** varias series por gráfico, leyenda, gráficos de área/radar/velas y
  eventos de clic en puntos o porciones.
