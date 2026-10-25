# ADR 0002 — Nodo `Canvas` (dibujo desde el cliente) — Spike M9

**Estado:** aprobada (2026-10-25) con el alcance recortado de §6.1. La especificación vigente es `docs/PROTOCOL.md` §20; donde este ADR difiera (§4, §6), manda §20.

## 1. Contexto

Hoy el cliente solo puede usar lo que el host ya mapea (componentes de gpui-kit). M9 busca subir el techo permitiendo que el cliente **dibuje desde el cliente** mediante un nodo `Canvas` que recibe una lista de comandos de dibujo en sus props, y el host los pinta con la API de pintura de GPUI.

Regla 2 del plan: no inventar APIs de GPUI. Regla 5: cualquier cambio de protocolo se propone y aprueba antes de programar.

## 2. API de pintura de GPUI 0.3.9 (confirmada en fuentes)

El host compila con `gpui-kit = "0.8"`, que reexporta `gpui` (gpui-pre 0.3.9). Se confirmó en el registro de Cargo:

- **Elemento de lienzo:** `gpui::canvas(prepaint, paint) -> Canvas<T>` implementa `Element`/`IntoElement`. Se puede insertar como cualquier nodo del árbol retenido.
  - `prepaint(bounds, window, cx) -> T`: se ejecuta una vez con los bounds del lienzo.
  - `paint(bounds, T, window, cx)`: se ejecuta cada frame.
- **Métodos de dibujo sobre `Window` (`impl Window` en `window.rs`):**
  - `paint_quad(PaintQuad)` — rectángulo relleno con borde/radios opcionales.
  - `paint_path(Path<Pixels>, color)` — cualquier path (líneas, polígonos, arcos).
  - `paint_image(...)` — imagen.
  - `paint_glyph(...)` — glifo/texto.
  - `paint_svg(...)` — SVG.
  - `paint_layer(bounds, f)` — capa de recorte/agrupación.
  - `paint_drop_shadows`, `paint_inset_shadows`.
- **Colores del tema:** el host ya recibe `&Theme` (gpui-component) en `render/mod.rs`, por lo que puede resolver nombres de tema (`primary`, `chart-1`, `background`, …) a color real al pintar. Sin nuevas dependencias.

**Conclusión técnica:** es factible un nodo `Canvas` que reciba una lista de comandos (rect, línea/path, texto, imagen) y los pinte con `window.paint_quad` / `paint_path` / `paint_image` / `paint_glyph`. No se necesitan APIs inexistentes.

## 3. Medición de coste: reenvío entero vs parche

Se midió con el host en release (`--no-window`) lanzando 50 commits por escenario sobre un nodo con una prop JSON grande que simula la lista de comandos de `Canvas`. Cada escenario lanza un proceso nuevo del host.

| Escenario | Comandos | Commits | ms/commit |
|-----------|----------|---------|-----------|
| full (entero) | 10 | 50 | 21.25 |
| patch | 10 | 50 | 20.90 |
| full (entero) | 100 | 50 | 21.66 |
| patch | 100 | 50 | 21.45 |
| full (entero) | 1000 | 50 | 21.23 |
| patch | 1000 | 50 | 21.37 |
| full (entero) | 5000 | 50 | 21.17 |
| patch | 5000 | 50 | 21.46 |
| baseline (vacío) | 0 | 50 | 21.19 |

**Lectura:** el tiempo por commit es ~21 ms en todos los casos, incluido el baseline vacío, y no varía con el número de comandos ni con entero/parche. El coste dominante es el arranque fijo del host (carga de GPUI) y el overhead de transporte, no el tamaño de la lista.

**Limitación:** la prop `commands` era desconocida para `div`, así que el host solo la almacena sin interpretarla. La medición cubre transporte + almacenamiento de la prop, **no** el coste de pintura real (eso requiere `Canvas` implementado y se medirá en fase 3).

**Pintura real (fase 3, 2026-10-25):** `node examples/canvas.ts --stress 5000` en ventana real (Windows, host de `cargo build`): 5005 comandos, y cada clic reenvía la lista entera (~290 KB de JSON) y la repinta. Tras varios clics no se percibe retraso. Es una prueba a ojo, no un número: no se instrumentaron los frames del host porque el resultado no cambia ninguna decisión de M9. La lista entera se queda; un parche sigue siendo posible sin romper el protocolo si algún caso real lo pide.

**Implicación:** a nivel de transporte/almacenamiento no hay argumento de peso para exigir un parche/diff. Enviar la lista entera en cada commit es aceptable. Si en fase 3 se mide que el renderizado de N comandos es costoso, se podrá añadir parche después sin romper el protocolo.

## 4. Diseño propuesto de comandos (forma del JSON)

`Canvas` recibe las props compartidas (`class`, `a11y`) más una lista `commands` y opcionalmente `events` para puntero con coordenadas locales:

```json
{
  "op": "create", "id": 10, "type": "Canvas",
  "props": {
    "class": "flex-1 border",
    "commands": [
      { "op": "rect",        "x": 10, "y": 10, "w": 100, "h": 60, "color": "primary",   "corner_radius": 8 },
      { "op": "line",        "x1": 0, "y1": 0, "x2": 200, "y2": 200, "color": "danger",  "width": 2 },
      { "op": "path",        "d": "M10 10 L90 10 L90 90 Z", "color": "chart-2", "width": 3 },
      { "op": "text",        "x": 20, "y": 40, "content": "Hola", "color": "foreground", "size": 16 },
      { "op": "image",       "x": 0, "y": 0, "src": "icon.png",   "fit": "contain" }
    ]
  },
  "events": ["pointerDown", "pointerMove", "pointerUp", "click"]
}
```

- `color`: nombre de color del tema resuelto por el host.
- Eventos de puntero con `x`/`y` locales del `Canvas`.
- Comandos malformados -> `invalid_props`.

## 5. `PROTOCOL_VERSION`

Se **mantiene en 1**. Añadir `Canvas` como nodo nuevo no rompe a hosts/clientes v1 (un host v1 responde `unknown_type` a `Canvas`; un cliente v1 no puede crear `Canvas`). Solo haría falta subir la versión si se cambia la semántica de mensajes existentes, cosa que no se hace. Es coherente con M2-M8, que añadieron nodos sin subir la versión.

## 6. Recomendación

**Proceder a la fase 2:** redactar la sección de `Canvas` en `docs/PROTOCOL.md` (propuesta de protocolo) y hacerla aprobar antes de programar. Si se aprueba, implementar en fase 3:

- Host: `schema.rs` (`NodeType::Canvas`, `PropKind` lista de comandos), `components/canvas.rs` (nuevo, pintar con API de GPUI), `render/mod.rs` (rama en `node_raw`).
- Clientes: Node (`protocol.ts`, `app.ts`), React (`components.ts`, `host-config.ts`), Python (`__init__.py`), Go (`nodes.go`).
- Conformance: `conformance/canvas.txt`.
- Ejemplo: gráfico simple de prueba de concepto que dibuje y responda al puntero.
- Prueba en ventana real (Windows; macOS/Linux pendientes como en M5).
- `HISTORIAL.md`, `ROADMAP.md`.

## 6.1. Recorte de alcance (fase 3, 2026-10-25)

El criterio de aceptación de M9 es "un ejemplo que dibuja y responde al puntero", así que M9 se limita a lo que lo cumple. `PROTOCOL.md` §20 es la referencia:

- Comandos: `rect`, `line`, `path` (`d` solo con `M`, `L` y `Z` absolutos).
- Evento: `pointerDown` con `value: {x, y}` en coordenadas locales.
- Fuera de M9 (cada uno es trabajo propio): `text` (necesita dar forma al texto antes de `paint_glyph`), `image` (necesita cargar la imagen), `pointerMove`, `pointerUp`, `click`.

## 7. Archivos relacionados

- `docs/PROTOCOL.md` — especificación (sección nueva de `Canvas`).
- `packages/host/src/schema.rs`, `packages/host/src/components/canvas.rs` (nuevo), `packages/host/src/render/mod.rs`.
- `packages/client-node/src/protocol.ts`, `packages/client-node/src/app.ts`.
- `packages/client-react/components.ts`, `packages/client-react/host-config.ts`.
- `packages/client-python/gpui_bridge/__init__.py`.
- `packages/client-go/gpuibridge/nodes.go`, `packages/client-go/gpuibridge/protocol.go`.
- `conformance/canvas.txt` (nuevo), `examples/canvas.ts` (nuevo).
- `docs/HISTORIAL.md`, `ROADMAP.md`.
