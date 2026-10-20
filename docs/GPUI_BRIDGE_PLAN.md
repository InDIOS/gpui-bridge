# gpui-bridge — Plan de implementación

> Documento de contexto para el asistente de IA que trabaja en este repositorio.
> Léelo completo antes de escribir código y síguelo fase por fase.

## 1. Qué estamos construyendo

Una librería que permite construir interfaces de escritorio **nativas** usando **GPUI** (el framework de UI del editor Zed, en Rust) y **gpui-component** (librería de componentes estilo shadcn para GPUI), pero escribiendo la **lógica de la aplicación en otro lenguaje**. El primer cliente fue **Node.js con TypeScript**; después se agregaron txiki.js, React y Python.

La idea central:

- Un **host en Rust** es dueño de la ventana, el loop de eventos, el árbol de componentes y el renderizado.
- Un **cliente** (Node) describe la interfaz mediante **operaciones sobre un árbol** (crear nodo, cambiar props, agregar hijo, eliminar) y recibe **eventos** (click, change, etc.).
- Se comunican por **stdin/stdout** usando **JSON Lines** (un mensaje JSON por línea).
- Los estilos se expresan como **strings de clases tipo Tailwind** (`"flex gap-2 p-4"`), que el host traduce a los métodos de estilo de GPUI.

### Estado

Hecho (ver §6): host con 40 tipos de nodo, varias ventanas, ventanas sin marco, temas, menús,
bandeja del sistema, animaciones, formularios, teclado y foco, diálogos nativos y portapapeles,
diálogos y avisos, lista virtualizada y tablas, accesibilidad (`a11y` en todo nodo) y los
componentes de gpui-kit de las categorías Basic, Form, Layout y Advanced que no piden
dependencias extra; clientes de Node, txiki.js, React y Python; transcripciones de conformidad y
empaquetado local (tarball de npm y wheel).

Fuera de alcance por ahora: plataformas distintas de Windows (GPUI las soporta, pero el host no
se probó ahí) y componentes definidos desde el cliente.

## 2. Reglas para el asistente de IA

1. **Trabaja una fase a la vez.** No empieces la siguiente fase hasta que se cumplan todos los criterios de aceptación de la actual. Al terminar una fase, resume qué se hizo y qué se verificó.
2. **No inventes APIs de GPUI ni de gpui-component.** Ambas cambian con frecuencia y la documentación es escasa. Antes de usar una función, tipo o trait, verifica que exista en la versión instalada: revisa el código fuente en el registro de Cargo (`%CARGO_HOME%\registry` o `%CARGO_HOME%\git`, ver sección 3), los ejemplos del repositorio de gpui-component y, si hace falta, el código de Zed. Si no puedes confirmarlo, dilo y pregunta.
3. **stdout es exclusivo del protocolo.** En el host, nunca uses `println!` ni escribas a stdout fuera del módulo de transporte. Todos los logs van a **stderr** (usa `eprintln!` o un logger configurado para stderr). Un print accidental rompe la comunicación.
4. **El host nunca debe cerrarse por un mensaje inválido.** Ante un mensaje mal formado, un nodo inexistente o una prop inválida, responde con un mensaje `error` y sigue funcionando.
5. **`protocol.rs` (host) y `protocol.ts` (cliente) son la fuente de verdad del protocolo.** Si cambias el protocolo, actualiza los dos, los otros clientes, [PROTOCOL.md](PROTOCOL.md) y sus transcripciones de `conformance/` en el mismo cambio. Un cambio de protocolo se propone y se aprueba antes de programarlo.
6. **Cambios pequeños y verificables.** Prefiere commits chicos, uno por paso, con mensajes descriptivos, en la rama local `dev` (sin push). Al terminar un paso, anótalo en [HISTORIAL.md](HISTORIAL.md) y espera la revisión antes de seguir.
7. **Prueba en ventana real.** Los tests corren sin ventana; lo que se ve (dibujo, mouse, teclado, foco) se verifica abriendo el ejemplo, y el historial dice qué se probó y qué no.
8. **Las versiones de `gpui` y `gpui-component` deben ser compatibles entre sí.** Usa la versión o revisión de `gpui` que indique gpui-component en su README o en su `Cargo.toml`, no necesariamente la última.
9. **Idioma.** Código, comentarios, mensajes, PROTOCOL.md y el README van en inglés; este plan y el historial, en español.

## 3. Requisitos del entorno

- Rust estable instalado con `rustup` (`rustc --version`, `cargo --version`).
- Node.js 20 o superior y npm o pnpm.
- Dependencias de sistema de GPUI según plataforma:
  - **macOS:** Xcode completo (GPUI usa Metal).
  - **Windows:** Visual Studio Build Tools con el componente de C++ (MSVC).
  - **Linux:** librerías de desarrollo de Vulkan, Wayland/X11 y `libxkbcommon`. El repositorio de Zed tiene un script que las instala por distribución; úsalo como referencia.
- La primera compilación de GPUI tarda varios minutos. Es normal.

### 3.1 Instalación en esta máquina (Windows)

Regla: todas las herramientas van en `D:\DEVELOP\Tools` para no ocupar el disco del sistema (`C:`).

Estado actual:

| Herramienta | Estado |
|-------------|--------|
| Node.js 24 | Instalado, gestionado con `mise` (`D:\DEVELOP\Tools\mise`) |
| pnpm | Instalado |
| Git | Instalado en `D:\DEVELOP\Tools\Git` |
| Rust | Instalado (1.99.1, `stable-x86_64-pc-windows-msvc`) en `D:\DEVELOP\Tools\rust` |
| VS Build Tools (MSVC + Windows SDK) | Instalado (MSVC 14.44) en `D:\DEVELOP\Tools\VS\BuildTools` |

Los comandos son de PowerShell.

**Paso 1 — Rust (todo en `D:`)**

```powershell
# Rutas de rustup y cargo en D: (variables de usuario permanentes + sesión actual)
[Environment]::SetEnvironmentVariable('RUSTUP_HOME', 'D:\DEVELOP\Tools\rust\rustup', 'User')
[Environment]::SetEnvironmentVariable('CARGO_HOME',  'D:\DEVELOP\Tools\rust\cargo',  'User')
$env:RUSTUP_HOME = 'D:\DEVELOP\Tools\rust\rustup'
$env:CARGO_HOME  = 'D:\DEVELOP\Tools\rust\cargo'

# Descargar e instalar (rustup agrega %CARGO_HOME%\bin al PATH de usuario)
Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile $env:TEMP\rustup-init.exe
& $env:TEMP\rustup-init.exe -y --default-toolchain stable --default-host x86_64-pc-windows-msvc
```

Con esto, el registro de crates y los checkouts git (`%CARGO_HOME%\registry`, `%CARGO_HOME%\git`) también quedan en `D:`. El directorio `host\target` vive dentro del proyecto, que ya está en `D:`.

**Paso 2 — Visual Studio Build Tools (mayormente en `D:`)**

```powershell
Invoke-WebRequest https://aka.ms/vs/17/release/vs_BuildTools.exe -OutFile $env:TEMP\vs_BuildTools.exe
& $env:TEMP\vs_BuildTools.exe --passive --wait --norestart `
  --installPath D:\DEVELOP\Tools\VS\BuildTools `
  --path cache=D:\DEVELOP\Tools\VS\Cache `
  --path shared=D:\DEVELOP\Tools\VS\Shared `
  --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended
```

- `--path cache` y `--path shared` solo se pueden elegir en la primera instalación de Visual Studio en la máquina.
- Aun así, una parte va a `C:` y no se puede mover: el instalador, el Windows SDK (`C:\Program Files (x86)\Windows Kits`) y los runtimes. Son unos 2–4 GB (estimado).
- La toolchain GNU (por ejemplo `w64devkit`) no sirve: GPUI necesita MSVC.

**Verificación** (en una terminal nueva)

```powershell
rustc --version
cargo --version
rustup show          # el host debe ser x86_64-pc-windows-msvc
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -products * -property installationPath
node --version       # 20 o superior
pnpm --version
```

Si al compilar aparece un error de `link.exe`, revisa que el PATH no ponga el `link.exe` de Git (`usr\bin`) o de `w64devkit` delante del de MSVC.

## 4. Estructura del repositorio

```
gpui-bridge/                     # raíz = orquestador (workspaces + scripts + CI)
├── Cargo.toml                   # workspace de Cargo y [profile.release] del host
├── package.json                 # scripts raíz: bootstrap, dev, test, pack:node, wheel
├── pnpm-workspace.yaml          # client-node, client-react y examples
├── README.md                    # qué es y cómo empezar (inglés)
├── CONTRIBUTING.md              # entorno, layout y chequeos de un cambio
├── CHANGELOG.md                 # versionado lockstep de host y clientes
├── LICENSE
├── docs/
│   ├── PROTOCOL.md              # especificación del protocolo (inglés)
│   ├── GPUI_BRIDGE_PLAN.md      # este documento
│   ├── HISTORIAL.md             # qué hizo cada fase y cómo se verificó
│   ├── GPUI_BRIDGE_COMPONENTS.md
│   ├── RELEASING.md             # cómo se corta una versión y qué lleva cada paquete
│   ├── adr/0001-monorepo.md     # por qué está así
│   └── GUIA-ESTRUCTURA.md       # propuesta original (aplicada)
├── scripts/
│   ├── stage-host.mjs           # copia el host ya compilado a los assets de los clientes
│   └── dev.mjs                  # `pnpm dev <ejemplo>`
├── benchmarks/                  # mediciones de transporte, árbol grande e imágenes
├── conformance/                 # transcripciones del protocolo (las ejecuta packages/host/tests/conformance.rs)
├── examples/                    # un ejemplo por tema + react/ (JSX); consumen los paquetes
└── packages/                    # todo lo distribuible
    ├── host/                    # crate de Rust (binario gpui-bridge-host)
    │   ├── Cargo.toml
    │   ├── tests/               # conformance.rs (transcripciones) y no_window.rs
    │   └── src/
    │   │   ├── main.rs              # arranque: app GPUI, lector de stdin, sesión y despacho de mensajes
    │   │   ├── transport.rs         # escritura a stdout (JSON Lines); nadie más escribe ahí
    │   │   ├── protocol.rs          # tipos de mensajes (serde)
    │   │   ├── schema.rs            # tipos de nodo y sus props: qué aceptan create y setProps
    │   │   ├── tree.rs              # árbol retenido de nodos, ventanas, tray y atajos; se aplica en commit
    │   │   ├── windows.rs           # ventanas de la plataforma (abrir, actualizar, cerrar) y tema
    │   │   ├── render/              # vista raíz de cada ventana: recorre el árbol y arma los elementos GPUI
    │   │   │   ├── mod.rs           # RootView, estados por nodo, lista virtualizada
    │   │   │   └── dialog.rs        # nodos Dialog en la capa de diálogos de gpui-component
    │   │   ├── styles.rs            # clases tipo Tailwind a estilos GPUI
    │   │   ├── animation.rs         # transition, animate-spin y animate-pulse
    │   │   ├── keys.rs              # atajos y teclas
    │   │   ├── system.rs            # acciones del sistema: diálogos nativos, portapapeles, abrir, notify
    │   │   ├── tray.rs              # icono y menú de la bandeja (tray-icon)
    │   │   ├── simulate.rs          # `simulate` (solo --no-window): eventos para las transcripciones
    │   │   └── components/          # adaptadores a gpui-component, uno por tipo de nodo
    │   │       ├── mod.rs           # eventos compartidos y modos sincronizado/controlado
    │   │       ├── button.rs, checkbox.rs, switch.rs, radio.rs, tabs.rs
    │   │       ├── input.rs         # Input y Textarea
    │   │       ├── select.rs, slider.rs, table.rs
    │   │       └── menu.rs          # menús contextuales y MenuBar
    ├── client-node/             # paquete `gpui-bridge` (TypeScript)
    │   ├── src/
    │   │   ├── index.ts             # API pública para Node (createApp)
    │   │   ├── app.ts               # núcleo independiente del runtime: App, UINode, handshake
    │   │   ├── txiki.ts             # API pública para txiki.js (createTxikiApp)
    │   │   ├── protocol.ts          # tipos de mensajes
    │   │   ├── host-process.ts      # transporte de Node: lanza el host y maneja stdin/stdout
    │   │   └── tree.ts              # ids y batching de operaciones
    │   └── test/client.test.ts
    ├── client-react/            # paquete `gpui-bridge-react`
    │   ├── src/
    │   │   ├── index.ts         # render()
    │   │   ├── components.ts    # Div, Text, Button, Input… tipados para JSX
    │   │   └── host-config.ts   # configuración de react-reconciler
    │   └── test/render.test.tsx
    └── client-python/           # paquete `gpui-bridge` (solo stdlib)
        ├── gpui_bridge/__init__.py  # App, Node, handshake y transporte
        ├── pyproject.toml
        ├── build_wheel.py       # wheel con el host optimizado dentro
        └── test_gpui_bridge.py
```

Los ejemplos React viven en `examples/react/`, no dentro del paquete: los ejemplos se ejecutan
como los consumiría un usuario (`pnpm dev`, `pnpm dev react/todo`).

## 5. Protocolo

La especificación (versión 1) está en [PROTOCOL.md](PROTOCOL.md), en inglés, para quien escriba un cliente en otro lenguaje. Las referencias "PROTOCOL.md §N" de este documento apuntan a sus secciones. Las transcripciones de [conformance/](../conformance/) son ejemplos ejecutables de esa especificación (PROTOCOL.md §9).

## 6. Historial

Lo hecho está en [HISTORIAL.md](HISTORIAL.md): criterios de aceptación, decisiones y qué se
verificó en cada fase. Índice:

- Fase 0 — Configuración
- Fase 1 — Host mínimo
- Fase 2 — Protocolo
- Fase 3 — Árbol retenido
- Fase 4 — Render de `div` y `text`
- Fase 5 — Traducción de clases
- Fase 6 — Componentes de gpui-component
- Fase 7 — Eventos
- Fase 8 — Cliente Node
- Fase 9 — Ejemplos
- Fase 10 (opcional) — Renderer de React
- Extra — Cliente para txiki.js y tamaño de distribución
- Fase 11 — Cliente de Python
- Extra — PROTOCOL.md y transcripciones de conformidad
- Fase 12 — Temas personalizados
- Fase 13 — App real: explorador de archivos
- Fase 14 — Scroll, clases de la app real e iconos
- Fase 15 — Tests automáticos de eventos
- Fase 16 — Empaquetado local
- Fase 17 — Imágenes
- Fase 18 — Ventanas sin marco
- Fase 19 — Varias ventanas
- Fase 20 — Menús
- Fase 21 — Bandeja del sistema
- Fase 22 — Animaciones
- Fase 23 — Formularios
- Fase 24 — Teclado
- Fase 25 — Diálogos nativos y sistema
- Fase 26 — Diálogos, notificaciones y tooltips
- Fase 27 — Lista virtualizada, `Table` y `Tabs`
- Extra — Rendimiento: redibujado incremental
- Extra — Accesibilidad y componentes de gpui-kit (Basic, Form, Layout y Advanced)

## 7. Decisiones de diseño (no cambiar sin discutirlo)

- **gpui-bridge y no `gpui-shell`.** `gpui-shell` (en el repo gpui-kit) ejecuta JavaScript con QuickJS dentro del proceso. El objetivo de gpui-bridge es una interfaz común para varios lenguajes, con Node.js real como primer cliente. Se pueden tomar ideas de `gpui-shell` (inventario de componentes, tokens del tema), pero no se depende de él.
- **Host como proceso hijo, no como librería nativa de Node.** Aísla los crashes, evita conflictos con el hilo principal y hace el protocolo agnóstico al lenguaje.
- **JSON Lines antes que formatos binarios.** Prioriza depurabilidad. Se puede migrar a MessagePack más adelante sin cambiar la arquitectura.
- **IDs asignados por el cliente.** Evita viajes de ida y vuelta para crear nodos.
- **Batching con `commit`.** El host nunca pinta estados intermedios.
- **Estado de inputs en el host.** Evita latencia al escribir.
- **Colores del tema, no paleta de Tailwind.** Mantiene coherencia con gpui-component y soporte de modo claro/oscuro.

## 8. Próximos pasos

Acordado el 2026-10-19: primero se cierran los cabos sueltos que quedaron sin verificar (lista y
estado en [HISTORIAL.md](HISTORIAL.md), "Extra — Cabos sueltos"), de a uno y con revisión entre
cada uno; recién después se elige de esta lista. Se para después de cada iteración.

Sin orden acordado para el resto; se eligen con el usuario y se hacen con las reglas de la §2.

> **Componentes que faltan:** el 2026-10-20 se documentó por qué `Dock`, `Command`, `Settings`,
> `Tree`, `List`, `Carousel`, `Diff`, `Chart` y `Sidebar` no están todavía, qué costaría cada uno
> y su impacto en host, protocolo y clientes: ver
> [GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md) (con el plan por tandas). Resumen: no
> falta tamaño sino modelo; `Editor` y `Speech` siguen fuera por dependencias.

- Componentes: **hecho el 2026-10-20** (HISTORIAL.md, "Accesibilidad y componentes de gpui-kit"):
  Basic (`Alert`, `Badge`, `Tag`, `Kbd`, `Progress`, `Spinner`, `Skeleton`, `Rating`,
  `Pagination`, `Collapsible`, `Accordion`, `Avatar`, `Stepper`), Form (`NumberInput`,
  `OtpInput`, `Form`, `Field`), Layout (`GroupBox`, `DescriptionList`, `StatusBar`, `Toolbar`,
  `Resizable`) y Advanced (`Calendar`), con `Accordion` y el redimensionado dentro de lo que el
  árbol puede expresar. Lo que queda se detalla en
  [GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md): `Tree`, `Diff` y `Carousel` son los
  siguientes por relación valor/coste; después `Sidebar` y `Chart`; `List`, `Command` y
  `Settings` según necesidad; `Dock` solo con un ADR. El editor de código (pide `tree-sitter`,
  que engorda el host), markdown (`TextView`) y el selector de color siguen pendientes.
- Ventanas: ocultar y mostrar ventanas sin cerrarlas, posición inicial y tamaño mínimo, eventos de foco y redimensionado de ventana.
- Rendimiento: redibujar solo los nodos que cambiaron (atajo de la fase 22). Medido el 2026-10-19 (HISTORIAL.md, "Medición"): el transporte no es el cuello de botella, el host aplica ~25 µs por nodo y cualquier animación satura un núcleo; el `batch` de operaciones no hace falta. **Hecho el 2026-10-20** (HISTORIAL.md, "Extra — Rendimiento: redibujado incremental"): un nodo con `animate-*` se dibuja en una vista propia, los cuadros van a lo sumo a 60 por segundo y el texto no se realoca por cuadro; la ventana en reposo sigue sin gastar CPU. Lo que queda: GPUI marca como sucios también los ancestros de la vista que se notifica, así que la ventana sigue recorriendo el árbol en cada cuadro (~54 ms por cuadro con 20 000 nodos en debug); con `VirtualList` el recorrido es de las filas visibles.
- Imágenes remotas: no se agrega HTTP al host (la decodificación pesa un orden de magnitud más que el transporte). El cliente baja el archivo y manda una ruta —lo más barato— o los bytes en una línea si hace falta. **Hecho el 2026-10-20** (HISTORIAL.md, "Extra — Imágenes por bytes"): `Image.data` acepta base64 estándar; el host lo decodifica una vez a un archivo por nodo (mismos formatos y animaciones que `src`) y lo borra con el nodo o con el árbol. `src` sigue siendo lo más barato para archivos locales.
- Accesibilidad: **hecho el 2026-10-20** (HISTORIAL.md, "Accesibilidad y componentes de gpui-kit"): todo nodo acepta `a11y` (`id`, `role`, `label`, `description`, `keyshortcuts` y el estado ARIA: `checked`, `selected`, `expanded`, `disabled`, `value`, `min`/`max`/`step`, `level`, `orientation`), validado y aplicado a la tecnología asistiva; la raíz de la ventana se informa como `window` con su título, que era el aviso de GPUI. Lo que queda: un repaso de lo que anuncia un lector de pantalla real (no hay uno en el entorno de pruebas) y roles automáticos más finos en los componentes que no dejan ponerlos desde fuera.
- Distribución: etiqueta de plataforma del wheel (hoy queda `py3-none-any` aunque lleva un binario de Windows) y publicación en npm/PyPI (solo con visto bueno: es un recurso remoto).
- Plataformas: compilar y probar en macOS y Linux.
