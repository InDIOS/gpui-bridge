# gpui-bridge — Componentes que faltan: por qué y qué costarían

> Documento de trabajo, 2026-10-20. En español, como [GPUI_BRIDGE_PLAN.md](GPUI_BRIDGE_PLAN.md)
> y [HISTORIAL.md](HISTORIAL.md); el protocolo y el README siguen en inglés (regla 9 del plan).
> Complementa la §8 del plan. La tanda 1 (`Tree`, `Diff`, `Carousel`) y la tanda 2
> (`Sidebar`, `Chart` de líneas, barras y tarta) están implementadas (HISTORIAL.md, «Fase 29» y
> «Fase 30»); el análisis de abajo sigue siendo el de las tandas 3–5.

## 1. Resumen

El 2026-10-20 se mapearon 23 componentes de gpui-kit (Basic, Form, Layout y parte de Advanced).
Quedaron fuera `Editor` y `Speech` por sus dependencias (correcto), pero también `Dock`,
`Command`, `Settings`, `Tree`, `List`, `Carousel`, `Diff`, `Chart` y `Sidebar`, y ahí la frase
"necesitan delegates o máquinas de estado propias" fue **imprecisa**: se pueden agregar. Lo que
no encaja es el modelo actual del bridge —*un nodo = props planas + hijos*—, y cada uno rompe un
punto distinto de ese modelo. Este documento deja ese análisis por escrito para retomarlo.

## 2. Dos criterios distintos

| Criterio | Qué significa | Componentes |
|---|---|---|
| **Dependencias** | Añadir features con crates que pesan MB en el host | `Editor` (`tree-sitter` + gramáticas), `Speech` (`cpal` / reconocedor del sistema) |
| **Modelo** | El componente no se describe con props planas + hijos; hay que ampliar el protocolo | `Dock`, `Command`, `Settings`, `Tree`, `List`, `Carousel`, `Diff`, `Chart`, `Sidebar` |

Ninguno del segundo grupo añade dependencias: `chart`/`plot` son matemática propia (gpui-base),
y `dock`, `setting`, `command`, `sidebar`, `list`, `diff` y `carousel` usan lo que
`gpui-component` ya trae compilado (serde, uuid, similar, ropey…). Con LTO, lo que no se use se
descarta; el binario solo crece por el código efectivamente enlazado.

## 3. Qué pide cada componente (verificado en gpui-component 0.7.3)

| Componente | Lo que pide su API | Qué le falta al bridge | Esfuerzo |
|---|---|---|---|
| `Tree` | `TreeItem { id, label, children, expanded, disabled }`; `TreeState::new(cx).items(..)`, `set_selected_item`; `TreeEvent::{Expanded, Collapsed}` (`gpui-base/src/tree.rs`) | **Nada estructural: son datos.** Props anidadas (`items` en árbol) y reaplicar selección/expansión, porque `set_items` las resetea | **S** |
| `Diff` | `DiffFile::parse(patch) -> Vec<DiffFile>`; `DiffState::new(files, cx)`; `Diff::new(&state)` con `mode` (unified/split), líneas, etc. | Un `patch` de texto y tratar el error de parseo como `invalid_props`. Cachear por hash del texto | **S–M** |
| `Carousel` | `CarouselState::new(item_count)` + `with_selected_index/axis/looping`; `CarouselContent`, `CarouselItem`, `Previous/Next/Pagination` | Un slot como el de `Tabs`/`Calendar`; hijos = diapositivas; `value` = índice | **S** |
| `Sidebar` | `Sidebar<SidebarItem>` con `SidebarMenuItem { label, icon, active, default_open }` y `SidebarGroup`; `collapsed`/`collapsible` | Datos (`items` de secciones y entradas) + `value` (id activo) + `change` | **S–M** |
| `Chart` | Siete builders genéricos por tipo: `LineChart<T,X,Y>::new(data).x(..).y(..)`, `BarChart`, `AreaChart`, `PieChart`, `RadarChart`, `CandlestickChart`, `SankeyChart` | Props de series (datos, ejes, colores) y sustituir los closures de tooltip por datos. Es "solo" datos, pero cada tipo tiene genéricos distintos | **M–L** |
| `List` | Trait `ListDelegate`: `items_count`, `render_item` (devuelve un elemento), `confirm`, `perform_search`, `load_more`, secciones… | Un **delegate genérico en el host** que renderice un nodo hijo por fila (la técnica de `VirtualList`) | **M** |
| `Command` | `CommandState` (`query`, `selected_index`, `matched_count`) + `CommandItem { label, icon, action: Box<dyn Action>, checked, keywords }` | Una `Action` de GPUI no existe en el bridge: se ignora y se emite el id elegido; además paleta modal (como `Dialog`) y filtrado | **M** |
| `Settings` | `Settings::pages([SettingPage { title, icon, groups: [SettingGroup { items: … }] }])` | Un mini-lenguaje de campos (label, tipo de control, valor) como `Table` con `columns`/`rows`, y `change` con `{key, value}` | **M–L** |
| `Dock` | `Panel: gpui_base::dock::Panel` (trait: `Render`, título, toolbar, menú), `DockArea`, splits, tab groups, layout **persistente** y drag&drop | **Se rompe el modelo**: los paneles son structs del host, el layout es jerárquico y persistente y el contenido de cada panel es un subárbol arbitrario | **L + ADR** |

Líneas de la librería (incluyen tests, para dimensionar el port): `diff` 7 784 · `chart` 5 937 ·
`carousel` 3 480 · `dock` 3 162 · `command` 2 955 · `setting` 2 190 · `list` 1 680 · `sidebar`
1 353 · `tree.rs` 118. El port en el host es mucho menor (un `components/<x>.rs` por componente),
pero el coste de integración lo marcan las APIs de la columna 2.

## 4. Por qué: el patrón del host y los cuatro puntos que rompen

Hoy un componente se agrega así (patrón ya usado por `Input`, `Select`, `Slider`, `OtpInput`,
`Calendar`): un **slot** por nodo en `RootView` (`HashMap<NodeId, XSlot>`), un `snapshot`/`sync`
que crea, actualiza o descarta el estado en cada commit, un `build` que traduce props a elemento,
y un evento opt-in. Ese patrón cubre "props planas + estado propio del control". Lo que los
nueve restantes agregan es alguna de estas cuatro cosas:

### 4.1 Delegados que construyen elementos (List)

`List` no recibe items: recibe un `ListDelegate` cuyos métodos (`render_item`, `confirm`,
`perform_search`) reciben `&mut Window, &mut App` y devuelven elementos. El host tendría que
implementar un delegate genérico que, para la fila `i`, renderice el hijo `i` del nodo. Es lo
que ya hace `render_rows` para `VirtualList`. **Riesgo:** cada closure nueva se reserva en la
pila por nivel de anidación; es el mismo motivo por el que `node_raw` se partió en una función
por tipo (`STATUS_STACK_OVERFLOW`, ver HISTORIAL.md).

### 4.2 Estado interno que se pierde al reconstruir (Tree, Diff, List)

`TreeState::set_items`, `DiffState::set_files` y el delegate de `List` **descartan** lo que el
usuario había hecho (selección, líneas seleccionadas, archivo colapsado). `Select` ya tiene este
problema resuelto a medias: el estado se reconstruye solo cuando cambian las `options`. Con
árboles y diffs hay que hacer lo mismo (comparar el prop, no reconstruir de más) y además
reaplicar la selección/expansión que el cliente impone. Con 10 000 items, reconstruir por commit
es caro.

### 4.3 Layout persistente con identidad de paneles (Dock)

`Dock` es el único que no es "un componente con props": es un **gestor de layout**. Los paneles
implementan un trait con `Render`, el área mantiene splits, grupos de pestañas y estado
serializable, y el usuario los arrastra. Para el bridge hay que decidir quién posee ese layout
(ver §10); ninguna de las dos opciones es un mapeo, es un cambio de modelo.

### 4.4 Datos con closures (Chart, Command)

`Chart` recibe `data` más closures (`x`, `y`, `tooltip_title`, `fill`). Traducibles a datos
("serie = puntos con x, y, color, nombre"), pero **cada tipo de gráfico tiene sus genéricos**
(`LineChart<T,X,Y>`, `BarChart<T,B,V>`, `PieChart<T>`…): siete mapeos distintos, no uno.
`CommandItem` incluye `action: Box<dyn Action>`, que en el bridge se sustituye por el id del
comando elegido en el evento.

## 5. Impacto en el host

- **Código**: un `components/<x>.rs` por componente, más su `NodeType`, sus `PropKind` (algunos
  recursivos), su rama en `node_raw` y su slot si tiene estado. Los de §3 marcados **S** son
  ~60–120 líneas, como `Calendar`; los **M–L** suman validadores y datos anidados.
- **`PropKind` recursivos**: hoy todos los tipos son escalares o listas planas (`Options`,
  `Columns`, `Rows`, `Entries`, `Items`). Un árbol (`Tree`), páginas/grupos (`Settings`) o
  series (`Chart`) agregan validadores propios con el mismo estilo de mensajes
  (`invalid_props` con el índice que falla).
- **Caché por contenido**: `Diff` parsea el patch y `Chart` recalcula escalas; hay que
  reconstruir solo cuando cambia el dato (comparar el prop como hace `Select` con `options`).
- **Eventos**: `Tree` necesita `expand`/`collapse` con id y `Settings` un `{key, value}`;
  `HostMessage::Event` solo tiene `value`, `checked` y `paths`. `value` admite objetos (ya se
  usa para `sort`), pero lo limpio es un campo `data` opcional.
- **Riesgo de regresión**: más superficie para que un mensaje inválido cierre el host (regla 4
  del plan): patch mal formado, id de panel duplicado, layout imposible.
- **Pila y rendimiento**: ver 4.1 y 4.2; todo componente nuevo con delegate o reconstrucción
  por commit debe probarse con árboles profundos y con listas grandes.

## 6. Impacto en el protocolo

- **Props anidadas** (árbol, páginas, series) y validación recursiva; el JSON de un `Tree` de
  10 000 items pesa como el `rows` de un `Table` de 10 000 filas, que ya se midió: el transporte
  no es el cuello de botella (~25 µs por nodo, HISTORIAL.md "Medición"), el límite es la pasada
  del host.
- **Eventos con payload objeto** (`expand`/`collapse`, `settings.change`).
- **Posible operación nueva solo para `Dock`** (leer/escribir el layout), que sí sería un cambio
  de protocolo a versionar.
- Todo lo demás es **compatible hacia atrás**: props nuevas y un `data` opcional no rompen a los
  hosts/clientes viejos (se ignoran los campos desconocidos, PROTOCOL.md §6), así que no hace
  falta subir `PROTOCOL_VERSION` salvo para `Dock`.

## 7. Impacto en los clientes

- **Node/txiki.js y Python**: el runtime no cambia (siguen siendo `create`/`setProps`/`on`); solo
  crecen los tipos (`TreeNode`, `ChartSeries`, `SettingPage`…). Los métodos nuevos siguen el
  formato de los existentes (`app.tree({...})`, `app.chart({...})`).
- **React**: sigue valiendo "1 elemento React = 1 nodo" mientras los datos vayan en props
  (`<Tree items={…}>`, `<Chart series={…}>`). `Dock` **no**: hay que decidir cómo se declaran los
  paneles (hijos con `id`/`title`, o un `render` por panel), porque el layout lo gestiona el
  componente.
- **Coste por render**: un `items` nuevo en cada render dispara un `setProps` con todo el árbol
  (igual que hoy con `rows`); las apps deben memoizar, y conviene documentarlo en cada ejemplo.


## 8. Tamaño del host

- **Ninguno de los nueve agrega dependencias.** Se eliminan con LTO si no se usan; el delta real
  se mide compilando una variante que los referencie y comparando el release (hoy ~15 MB, dice el
  README). Estimación de partida: decenas a pocos cientos de KB por los grandes (`Chart`, `Dock`,
  `Settings`, `Command`), no MB.
- **Los que sí pesan son `Editor` y `Speech`** (`tree-sitter` + gramáticas; `cpal` y el
  reconocedor del sistema): decenas de MB en el caso del editor si se habilitan varios lenguajes.
  Ese criterio ya se aplicó y no se revisa en este documento.
- Para dimensionarlo con datos: compilar `host` con una rama temporal que construya un
  `LineChart`/`DockArea` y comparar `target/release/gpui-bridge-host.exe` antes y después.

## 9. Plan propuesto por tandas

Cada tanda sigue las reglas del plan (§2): protocolo + clientes + conformidad + documentación en
el mismo cambio, prueba en ventana real y anotación en HISTORIAL.md.

| Tanda | Componentes | Por qué en este orden | Criterios de aceptación |
|---|---|---|---|
| **1** | `Tree`, `Diff`, `Carousel` — **hecha (2026-10-21)** | Los tres son datos + un slot; no tocan el modelo. Mejor relación valor/riesgo | props validadas (incluidos árboles y el patch), selección/expansión reaplicadas tras reconstruir, transcripciones propias, ejemplo y ventana real |
| **2** | `Sidebar`, `Chart` (`Line`, `Bar`, `Pie`) — **hecha (2026-10-21)** | Datos con más estructura; `Chart` se puede partir por tipo de gráfico | props validadas, transcripción `charts.txt`, ejemplo `dashboard.ts` con 1000 puntos y un árbol de 10 niveles, y ventana real |
| **3** | `Command` (**hecho**, M10 paso 1, `PROTOCOL.md` §21), `Settings` | Necesitan un mini-lenguaje de datos (como `Table`) y una paleta modal | igual, más `query`/filtrado y `change` con `{key, value}` |
| **4** | `List` | Solo si hace falta lo que `VirtualList` no da (secciones, búsqueda integrada, carga incremental) | delegate genérico probado con 10 000 filas y árbol profundo |
| **5** | `Dock` | Requiere un ADR antes de escribir código | ADR aprobado; después, lo mismo que las demás tandas + persistencia del layout |

**Subcomponentes — piloto con `Tab` dentro de `Tabs` (hecho el 2026-10-21, M4).** Un componente
cuyo contenido es una lista de props (`tabs`, `options`, `items`) no puede llevar nodos dentro.
El patrón de `Form`/`Field` sí: el padre mira a sus hijos y usa los del tipo del subcomponente
(`Tab` con `value`, `label`, `disabled`, y sus propios hijos dentro), e ignora su prop de lista
cuando los tiene. Sirve para que un `RadioGroup` tome `Radio` hijos, un `Stepper` `Step`, un
`DescriptionList` `Entry` y un `Accordion` `Section`, sin tocar el modelo del host (nodos
normales, `change` desde el padre). El del piloto quedó en `PROTOCOL.md` §15.

## 10. Decisiones abiertas (Dock)

1. **Quién posee el layout.** (a) El cliente manda un árbol de splits/pestañas con ids de panel y
   el host crea un panel contenedor por nodo; (b) el host guarda el layout y lo expone al cliente
   con una operación `dockLayout` (get/set). La (a) mantiene el cliente como fuente de verdad y
   encaja con el resto del protocolo; la (b) evita mandar el layout en cada commit.
2. **Identidad de paneles.** Un id estable por panel (`panelId`) para reconciliar al añadir,
   quitar o mover; sin él, el drag&drop del usuario y el `setProps` del cliente se pisan.
3. **Contenido de un panel.** Un subárbol del cliente (nodo hijo) o un nodo por panel con su raíz
   declarada en props. La primera opción es coherente con `Dialog`/`Collapsible`.
4. **Persistencia.** ¿Solo en memoria de la ventana (como las columnas de `Table` y los tamaños de
   `Resizable`) o también en disco, a cargo del cliente?

## 11. Referencias verificadas (2026-10-20, gpui-component 0.7.3)

- `src/list/delegate.rs` — `pub trait ListDelegate` (items_count, render_item, confirm, …).
- `src/list/list.rs` — `ListState::new(delegate, window, cx)`, `set_selected_index`, `set_query`.
- `gpui-base/src/tree.rs` — `TreeItem { id, label, children, expanded, disabled }`, `TreeEvent`,
  `TreeState::new/items/set_items/set_selected_item`.
- `src/diff/document.rs` — `DiffFile::parse(patch)`; `src/diff/state.rs` — `DiffState::new(files, cx)`.
- `src/carousel/state.rs` — `CarouselState::new(item_count)`, `with_selected_index/axis/looping`.
- `src/sidebar/menu.rs` — `SidebarMenu`, `SidebarMenuItem { label, icon, active, default_open }`;
  `src/sidebar/group.rs` — `SidebarGroup`.
- `src/chart/*.rs` — `LineChart<T,X,Y>`, `BarChart<T,B,V>`, `PieChart<T>`, etc.
- `src/command/item.rs` — `CommandItem { action: Box<dyn Action>, checked, keywords }`;
  `src/command/state.rs` — `CommandState`.
- `src/setting/page.rs`, `src/setting/settings.rs` — `Settings::pages([SettingPage…])`.
- `src/dock/panel.rs` — `pub trait Panel: gpui_base::dock::Panel`.

No implementado todavía: `Settings`, `List` y `Dock` (`Command` se hizo en M10), que son las tandas 3–5 (la lista
de tipos de nodo es ya la de PROTOCOL.md §5, 42 tipos).

