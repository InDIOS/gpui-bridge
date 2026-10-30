# ADR 0004: M18 — Inventario de ranuras como hijos y análisis de Questionnaire

- **Estado:** Aprobado (opción 1, 2026-10-30)
- **Fecha:** 2026-10-30
- **Hito:** M18 (Segunda fase: ranuras como hijos y protocolo)

---

## 1. Contexto y objetivo

El modelo de `gpui-bridge` es **un nodo = props planas + hijos**. Antes de la regla del
2026-10-29 («una ranura que acepta elementos va como hijo-nodo, salvo atajo justificado»), varios
nodos expusieron como texto o como datos ranuras que gpui-component 0.8.1 acepta como elemento
(`impl IntoElement`, `AnyElement` o `ParentElement`).

M18 tiene que:
1. Auditar los **76 tipos de nodo** de `PROTOCOL.md` §5.
2. Encontrar las ranuras de elemento que hoy van como texto o como datos.
3. Decir, para cada una, si cambiarla rompe clientes o sube `PROTOCOL_VERSION`, y qué cuesta en
   nodos por instancia.
4. Proponer qué se hace, sin programar hasta que el usuario lo apruebe.

**Método.** Se listaron todos los `pub fn …(impl IntoElement | AnyElement)` y los
`impl ParentElement for` de `gpui-component-0.8.1/src` y se cruzaron con lo que el host pasa en
`packages/host/src/components/*` y `render/*`. No se recorrieron otras firmas (por ejemplo
`impl Into<FieldBuilder>` o `DescriptionText`) salvo las que se nombran abajo.

---

## 2. Conclusión de protocolo

**Ningún caso necesita subir `PROTOCOL_VERSION`.** Todas las ranuras pendientes se pueden abrir
de forma aditiva: la prop de texto se queda y un hijo-nodo opcional (patrón `Tab`/`ListItem`/
`MessageHeader`) gana cuando está. Un cliente viejo sigue mandando la prop y sigue funcionando.

La premisa de M18 («lo que toca la versión del protocolo») queda vacía. Lo que queda son mejoras
aditivas, que la regla del ROADMAP permite hacer cuando haga falta, sin un hito propio.

---

## 3. Inventario

### 3.1. Ya son hijos-nodo (nada que hacer)

| Nodo | Ranura del crate | Cómo llega hoy |
|---|---|---|
| `div` | hijos | hijos |
| `Tabs` / `Tab` | `Tab` como `ParentElement`, `suffix` | `Tab` hijo de `Tabs`; los hijos del `Tab` van dentro del chip y un control final va a `suffix` |
| `List` / `ListItem` | `ListItem` como `ParentElement` | `ListItem` hijo con cualquier subárbol |
| `Dock` / `DockPanel` | contenido del panel | `DockPanel` hijo con contenido libre |
| `Message` | `MessageHeader`, `MessageContent`, `MessageFooter` | `MessageHeader`/`MessageFooter` como nodos; el resto de hijos es el contenido |
| `Bubble`, `MessageScroller` | `ParentElement` / filas | hijos |
| `Sheet` | cuerpo (`ParentElement`) | hijos (ver 3.3 para `title`/`footer`) |
| `Popover`, `HoverCard` | `trigger` + panel | primer hijo = disparador, el resto = panel |
| `Collapsible` | `content` | hijos |
| `Toolbar` | `content`/`contents` | hijos |
| `Carousel` | `CarouselItem` | un hijo por diapositiva |
| `Resizable` | paneles | un hijo por panel |
| `Badge` | `ParentElement` | hijos bajo la insignia |
| `Empty` | `EmptyContent` | hijos (ver 3.3 para la cabecera) |
| `Dialog` | cuerpo (`ParentElement`) | hijos (ver 3.3 para `title`/`footer`) |
| `Attachment` | `AttachmentActions` | hijos = botones de acción |
| `Marker` | `MarkerContent` (`ParentElement`) | texto de `content` y después los hijos |

### 3.2. Texto o datos a propósito (se queda así)

| Nodo | Ranura del crate | Hoy | Por qué se queda |
|---|---|---|---|
| `Button`, `Toggle`, `Checkbox`, `Switch`, `RadioGroup` | `ParentElement` (`Radio` y los demás) | `label` texto | El `label` es también el nombre accesible del control; con hijos cada cliente tendría que poner `a11y.label` a mano. El icono ya tiene su prop. |
| `Marker.content` | `MarkerContent` | texto + hijos detrás | El brillo de carga del crate solo se aplica a un `SharedString`. |
| `Message.avatar` | `impl IntoElement` | `avatarName`/`avatarSrc` | Decisión D2, opción C, de M15. |
| `Questionnaire` | `QuestionnaireItem`, `QuestionnaireChoice`… | `items` (datos) | Ver §4. |
| `Table` | `TableCell`/`TableHead` (`ParentElement`) | `columns`/`rows` (datos) | La tabla es virtual y la dibuja un delegate por fila; un subárbol por celda multiplica los nodos por fila. |
| `Tree`, `Sidebar` (items), `Command`, `Settings`, `Select`, `Combobox` (opciones), `MenuBar`, `DropdownButton` (menú), `Stepper` | ítems con `ParentElement` o `label` | datos | Son listas de datos que el crate dibuja con su propio estado (selección, búsqueda, teclado). Abrir cada ítem a nodos pasa por el mismo análisis que `List` (M15) y no hay pedido. |
| `Alert` | `title(impl Into<SharedString>)` | texto | El crate no acepta elementos. |

### 3.3. Ranuras de elemento que hoy van como texto o no existen (candidatas aditivas)

Coste en nodos: «+1 nodo» es un hijo-nodo de marcado (como `MessageHeader`) más su subárbol, solo
cuando el cliente lo usa. Sin el hijo el coste no cambia.

| Nodo | Ranura del crate | Hoy | Forma aditiva | Coste |
|---|---|---|---|---|
| `Accordion` | `AccordionItem` (`ParentElement`) y `title(impl IntoElement)` | `items[].content` y `.title` texto | hijo `AccordionItem` (ver §5) | +1 nodo por sección |
| `Dialog` | `title`, `footer` (`impl IntoElement`) | `title` texto; el pie lo arma el host con `okText`/`cancelText` | hijos `DialogTitle`/`DialogFooter` | +1 nodo por ranura |
| `AlertDialog` | `title`, `description`, `icon`, `footer` | `title`/`description`/`icon` texto; el pie lo arma el host | igual que `Dialog` | +1 nodo por ranura |
| `Sheet` | `title`, `footer` | `title` texto; sin pie | hijos de marcado | +1 nodo por ranura |
| `GroupBox` | `title`, `footer` | `title` texto; sin pie | hijos de marcado | +1 nodo por ranura |
| `Form` | `footer` | no existe | hijo de marcado | +1 nodo |
| `Field` | `label_fn`/`description_fn` (elementos) | `label`/`description` texto | hijos de marcado | +1 nodo por ranura |
| `StatusBar` | `left`, `right` (`impl IntoElement`) | `left`/`right` texto | hijos de marcado | +1 nodo por lado |
| `Sidebar` | `header`, `footer` | no existen | hijos de marcado | +1 nodo por ranura |
| `Input`, `NumberInput` | `prefix`, `suffix` | no existen | hijos de marcado | +1 nodo por ranura |
| `InputGroup` | `InputGroupAddon`/`InputGroupButton` | `prefix`/`suffix` texto y `prefixIcon`/`suffixIcon` | hijos de marcado | +1 nodo por ranura |
| `Tabs` | `TabBar.prefix`/`suffix` | no existen | hijos de marcado | +1 nodo por ranura |
| `Empty` | `EmptyMedia`, `EmptyTitle`, `EmptyDescription` | `icon`/`title`/`description` (decisión A1 de M12) | hijos de marcado | +1 nodo por ranura |
| `Attachment` | `overlay`, `AttachmentMedia`, elementos extra de `AttachmentContent` | `title`/`description` texto, imagen por `src`/`data` | hijos de marcado | +1 nodo por ranura |
| `DescriptionList` | `DescriptionText::AnyElement` | `items` texto | hijo por entrada | +1 nodo por entrada |
| `Tag` | `ParentElement` | `label` texto | hijos | 0 (los hijos reemplazan el texto) |
| `Combobox` | `footer` | no existe | hijo de marcado | +1 nodo |

**Correcciones al registro:**
- `HISTORIAL.md` (2026-10-20) dice que `StatusBar` usa `left`/`right` como texto «que es lo que
  expone el componente». En realidad el crate acepta `impl IntoElement`.
- La Fase 47 de `HISTORIAL.md` mandó a M18 el `overlay` de `Attachment` y los elementos extra de
  `AttachmentContent`. Por eso están en 3.3 y no en «excluido». Solo el título y la descripción
  quedan como texto por el brillo de carga.

---

## 4. `Questionnaire`

`QuestionnaireState` (gpui-base) lleva la pregunta actual, las respuestas, `required`, la
selección simple o múltiple y el teclado. Las opciones son `QuestionnaireChoiceDefinition` y el
texto libre usa un `InputState` por pregunta. Hoy `items` es
`{name, label, description?, required?, multiple?, disabled?, input?, choices?}`, y cada opción
es `{value, label, description?, disabled?, selected?}` (§26).

En el crate, `QuestionnaireChoice` es `ParentElement`: el **contenido** de la tarjeta puede ser
cualquier elemento sin tocar el estado, porque la tarjeta sigue siendo la del crate con sus
eventos. Lo que sí rompe `change`/`submit` es reemplazar la pregunta o la tarjeta entera por nodos
propios (la opción «`QuestionnaireCustomItem`» del borrador anterior).

**Decisión propuesta:** se queda como datos. Abrir el contenido de una opción a nodos es posible
y aditivo (un hijo por `value`), pero nadie lo pidió.

---

## 5. Diseño de la opción 1 (aprobada)

Los tres casos siguen el patrón de `MessageHeader`: un hijo-nodo de marcado opcional que gana sobre
la prop de texto; sin él, todo funciona como hoy. Un marcador fuera de su padre no dibuja nada.
Ninguno sube `PROTOCOL_VERSION`.

**`Accordion` / `AccordionItem`** (§17)
- `AccordionItem` hijo de `Accordion`, con `id` (obligatorio, no vacío), `title`, `open`,
  `disabled` y `class`. Se llama `id`, igual que en `items`, para que `change` mande lo mismo en
  los dos modos. Sus hijos son el contenido de la sección.
- Si `Accordion` tiene algún hijo `AccordionItem`, `items` se ignora (aviso una vez en el log).
- `change` manda el `id` de la sección; sincronizado, el host guarda el estado en el `open` del
  `AccordionItem` (en modo `items`, en `items` como hoy); `controlled` solo avisa.
- Coste: +1 nodo por sección, más su contenido.

**`Dialog` / `AlertDialog` + `DialogFooter`** (§14 y §24)
- Un hijo `DialogFooter` reemplaza el pie que arma el host: sus hijos (por ejemplo `Button`) son
  el pie; `okText` y `cancelText` se ignoran. Los botones mandan su propio `click` y el cliente
  cierra con `open: false`, que no manda nada. Escape, la X y un clic fuera siguen mandando
  `cancel`.
- El `DialogFooter` no es parte del cuerpo. El `title` queda como texto (la opción 1 abre solo
  el pie).
- Coste: +1 nodo, más los botones.

**`StatusBar` + `StatusBarLeft` / `StatusBarRight`** (§17)
- Un hijo `StatusBarLeft` reemplaza el texto `left`, y `StatusBarRight` el `right`; sus hijos van
  a esa zona de la barra. Los demás hijos siguen sin dibujarse, como hoy.
- Coste: +1 nodo por lado, más su contenido.

Por cada caso: host, `PROTOCOL.md`, conformance, los 4 clientes, ventana real y Narrator.

---

## 6. Opciones para M18

- **Opción 0 (recomendada): cerrar M18 sin código.** Ningún caso necesita subir la versión. Este
  ADR queda como inventario y cada candidata de 3.3 se hace cuando un ejemplo o un usuario la
  necesite, con la regla aditiva del ROADMAP. Se corrige la nota de `StatusBar` en el siguiente
  `HISTORIAL.md`.
- **Opción 1 (elegida por el usuario el 2026-10-30): M18 = las candidatas que más valen.** `Dialog`/`AlertDialog` `footer` (botones
  propios en lugar de solo Aceptar/Cancelar), `Accordion` (§5) y `StatusBar` `left`/`right`.
  Cada una es host + §n + conformance + 4 clientes.
- **Opción 2: M18 = toda la tabla 3.3.** Unos 17 nodos de marcado nuevos. No se recomienda: es
  mucho protocolo nuevo sin un uso que lo pida.
