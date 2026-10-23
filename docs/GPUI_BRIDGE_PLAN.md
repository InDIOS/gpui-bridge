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
dependencias extra; clientes de Node, txiki.js, React y Python; transcripciones de conformidad;
empaquetado por plataforma (un paquete npm por sistema, wheel con tag de plataforma) y el host
compilado y probado en CI en Windows x64, macOS arm64 y Linux x64 (glibc 2.35 o más nueva). En
ventana real (dibujo, clics, teclado) solo se probó en Windows.

Fuera de alcance por ahora: componentes definidos desde el cliente.

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
  - **Linux:** las bibliotecas de desarrollo de Vulkan, Wayland/X11, `libxkbcommon` y fontconfig.
    La lista exacta que instala CI (`apt-get`) está en `.github/workflows/ci.yml`; el repositorio
    de Zed tiene además un script que las instala por distribución, útil como referencia.
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

Desde el 2026-10-21 el orden lo marca la hoja de ruta de §8.1; lo que no está en ella (§8.2) se
elige con el usuario y se hace con las reglas de la §2.

### 8.1 Hoja de ruta (aprobada el 2026-10-21)

Estado de partida verificado el 2026-10-21: 48 tests unitarios del host, la transcripción de
conformidad y la prueba `no_window` en verde; 17 tests de Node, 14 de React y 17 de Python;
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` y el type-check de
los ejemplos, limpios. Los "cabos sueltos" de las fases 17–27 están cerrados
([HISTORIAL.md](HISTORIAL.md)). El repositorio ya tiene `origin`
(<https://github.com/InDIOS/gpui-bridge>).

El orden de la tabla está aprobado y se programa un hito por iteración, con revisión y parada
entre cada uno (regla 6). **Hechos: M0–M5. Siguiente: M6.** El 2026-10-21 se agregó M4
(subcomponentes) y los hitos siguientes pasaron a M5–M7. El 2026-10-23 se reordenó lo pendiente
a partir de un análisis de competencia: entraron M6 (gpui-kit 0.8), M8 (cliente Go) y M9
(`Canvas`), y la primera publicación y la tanda 3 pasaron a M7 y M10. Las entradas de
[HISTORIAL.md](HISTORIAL.md) anteriores a cada cambio conservan la numeración vieja.

| Hito | Contenido | Criterio de aceptación | Protocolo |
|---|---|---|---|
| **M0 — Estabilizar el repositorio** — **hecho** («Fase 28») | Lo hecho en este cambio: metadatos de publicación en los tres paquetes, enlaces al repositorio, `.gitattributes` (LF en todo el árbol), versión de Rust fijada en `rust-toolchain.toml` y en CI, `cargo fmt --check` y `clippy -D warnings` en CI, `--locked` en los comandos de Cargo, cancelación de runs viejos. Pendiente fuera del repo: protección de rama en `main`, secretos `NPM_TOKEN`/`TWINE_*` si se publica. | CI en verde en `dev` y `main`; `pnpm pack:node` y `pnpm wheel` producen artefactos que se instalan y abren un ejemplo **fuera** del repositorio. | sin cambios |
| **M1 — Ventanas** — **hecho** («Fase 28») | Ocultar y mostrar ventanas sin cerrarlas, posición inicial, tamaño mínimo, eventos de foco y de redimensionado de ventana. | `PROTOCOL.md` §3 actualizado, transcripción propia en `conformance/`, ejemplo en `examples/windows.ts` y prueba en ventana real. | sí: extiende `window` (§3); compatible hacia atrás si los campos son opcionales |
| **M2 — Componentes, tanda 1** — **hecho** («Fase 29») | `Tree`, `Diff`, `Carousel` ([GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md) §9). | Props validadas (árboles y patch incluidos), selección/expansión reaplicadas al reconstruir, transcripción, ejemplo y ventana real; clientes Node, React y Python en el mismo cambio. | no: props nuevas, sin subir `PROTOCOL_VERSION` |
| **M3 — Componentes, tanda 2** — **hecho** («Fase 30») | `Sidebar`, `Chart` (`Line`, `Bar`, `Pie`), con `xTicks`/`yTicks` para el número de etiquetas de cada eje. | Lo anterior más una prueba con series de 1000 puntos y árbol profundo. | no |
| **M4 — Subcomponentes: `Tab` dentro de `Tabs`** (piloto) — **hecho** («Fase 31») | Nodo `Tab` (`value`, `label`, `disabled`) cuyos hijos van dentro de la pestaña, con el patrón de `Form`/`Field`; un `Tabs` con hijos `Tab` los usa e ignora `tabs`, sin hijos funciona como hoy. `change` sigue saliendo de `Tabs`. Si sale bien, el mismo patrón sirve para `RadioGroup`, `Stepper`, `DescriptionList` y `Accordion`. | Pestañas hechas de nodos (icono + texto + `Badge`) que informan la elegida igual que hoy, por clic y por `simulate`; un `Button` dentro de una pestaña no la cambia; transcripción, ejemplo, los tres clientes y ventana real. | no: nodo nuevo, `tabs` sigue igual |
| **M5 — Multiplataforma** — **hecho** («Fase 32») | El host compila y se prueba en Windows, macOS y Linux (en Linux el icono de bandeja usa `ksni`, sin GTK). Un paquete npm por plataforma (`gpui-bridge-win32-x64`, `-darwin-arm64`, `-linux-x64`, en `packages/host-<plataforma>-<arch>`) lleva el binario y es `optionalDependencies` de `gpui-bridge`, que ya no lleva binario propio; el wheel sigue llevándolo con tag por plataforma; `os`/`cpu` fuera de los dos clientes npm. CI con matriz de los tres sistemas y empaquetado por plataforma. | CI con matriz de los tres sistemas; instalar el paquete y abrir un ejemplo en cada uno (`scripts/check-package.mjs` instala tarball y wheel **fuera** del repositorio, sin `GPUI_BRIDGE_HOST`, y abre una app headless con Node y Python); relajar `os`/`cpu` de los `package.json` junto con los assets. | no |
| **M6 — Actualizar a gpui-kit 0.8** | Subir de gpui-kit 0.7.3 (GPUI `=0.3.9`) a la 0.8 (2026-10-19) y adaptar el host a sus cambios incompatibles: variantes nuevas de `TabVariant`, Inicio/Fin y propagación en `DataTable`, rebote y rueda del scroll, `ResizeHandle::on_drag` con `&T` y el sobrepaso del easing. | El host compila con la 0.8, todos los tests en verde y `components`, `data`, `advanced` y `dashboard` se ven y se comportan como antes en ventana real (o el cambio queda documentado). | no |
| **M7 — Primera publicación** | Docs coherentes sobre plataformas (README y §1 de este plan dicen hoy cosas distintas); el README abre con Python y presenta el proyecto como «GPUI desde cualquier lenguaje que pueda lanzar un proceso»; versión semver (0.2.0 o 1.0.0, a decidir), `CHANGELOG.md`, npm y PyPI, firma de código del host de Windows. | Instalación desde los registros, sin `GPUI_BRIDGE_HOST`, abriendo un ejemplo; `gpui-bridge-host --version` coincide con `bin/host.json`; README y plan dicen lo mismo sobre plataformas. | no: el protocolo sigue en 1 |
| **M8 — Cliente Go** | `packages/client-go/` escrito solo con `PROTOCOL.md` y `conformance/` (sin dependencias: `encoding/json` y `os/exec`), `examples/counter.go`, job de CI en Windows y Linux; cada hueco de la especificación que obligue a mirar el host se corrige en el mismo hito. | El cliente reproduce todas las transcripciones y abre el contador en ventana real; la lista de huecos de `PROTOCOL.md` queda cerrada. | no: es la prueba de que el protocolo basta |
| **M9 — `Canvas`** | Spike de un nodo con una lista de comandos de dibujo (rectángulo, línea, path, texto, imagen, colores del tema) pintada con `canvas` de GPUI, midiendo reenviar la lista frente a un parche; después, propuesta de protocolo (regla 5) y, si se aprueba, host, clientes, transcripción y ejemplo. | Spike con números y decisión escrita; si sigue, un ejemplo que dibuja y responde al puntero sin componentes nuevos en el host. | sí: nodo y comandos nuevos, eventos de puntero con coordenadas locales |
| **M10 — Tanda 3 y cierre de calidad** | `Dock`, `Command`, `Settings`, repaso con un lector de pantalla real, recorrido de 20 000 nodos por cuadro, `benchmarks/tree.mjs` repetido con el host release y la latencia de aplicar 1k/10k/50k nodos en el README. | Lo de M2 más `change` con `{key, value}`; informe de lo que anuncia el lector y del coste por cuadro. | `change` con objeto (campo `data` opcional) |

> **Estado (2026-10-21):** **M0 y M1 hechos** ([HISTORIAL.md](HISTORIAL.md), «Fase 28»): CI en
> verde en los tres jobs y ventanas verificadas en ventana real. **M2 hecho** («Fase 29»):
> protocolo, host, los tres clientes, `conformance/advanced.txt`, ejemplo, 52+19+16+19 tests en
> verde y prueba en ventana real (selección y expansión del árbol, selección de líneas del diff y
> carrusel con flechas y puntos). **M3 hecho** («Fase 30»): `Sidebar` y `Chart` (con `xTicks` y
> `yTicks` para las etiquetas de los ejes), 58+21+17+21 tests en verde y prueba en ventana real
> con una serie de 1000 puntos y un árbol de 10 niveles. **M4 hecho** («Fase 31»): `Tab` dentro de
> `Tabs` (con `value` obligatoria, hijos después del `label`, `class` del chip y `a11y.label` para
> la pestaña sin `label`), 58+22+18+22 tests en verde, `conformance/data.txt` ampliado, los ejemplos
> `components.ts` y `data.ts`, y la comprobación visual del usuario. **M5 hecho** («Fase 32»):
> host compilable en los tres sistemas, un paquete npm por plataforma con el binario dentro,
> wheel con tag por plataforma, CI con matriz de tres sistemas (host en los tres, clientes en
> Windows y Linux) y empaquetado por plataforma con la prueba de instalar y abrir headless.
> M6 a M10 sin empezar. Proteger `main` queda fuera de M0, para cuando el repositorio sea
> público. La comprobación visual en ventana real de macOS y Linux sigue pendiente: no hay
> máquinas de esas plataformas en el entorno, así que M5 se cierra con la verificación de CI
> (decisión del usuario).

Por qué este orden: M0 deja el repositorio publicable y reproducible; M1 cierra el hueco más
grande del **núcleo** (la semántica de ventanas) antes de sumar superficie de componentes; M2 y
M3 son incrementales y de riesgo bajo; M4 va antes de M5 y de la publicación porque agrega API pública en los
clientes y conviene que entre en la primera release; M5 va **antes** de la publicación porque
publicar solo Windows desperdicia la primera release.

Orden de lo pendiente (2026-10-23): gpuix (React/Solid con napi) y gpui-shell (QuickJS, de los
autores de gpui-kit) ya cubren JS dentro del proceso, y no hay bindings de GPUI para Python ni
para otros lenguajes. Lo que diferencia al proyecto es el protocolo neutral, así que:
M6 va antes de M7 para que la primera release no nazca atrasada respecto de gpui-kit; M7 publica
con ese mensaje (Python primero); M8 demuestra con un cliente que no es JS que el protocolo basta,
y eso vale más que sumar componentes; M9 sube el techo de lo que el cliente puede dibujar, más que
`Dock`/`Command`/`Settings`; M10 es lo último porque necesita un lector de pantalla real y una
medición fina que hoy no están en el entorno.

Cada hito se trabaja como las fases anteriores: protocolo + host + los tres clientes +
`PROTOCOL.md` + `conformance/` + `HISTORIAL.md` en el mismo commit, prueba en ventana real de lo
que se dibuja o se toca, y parada para revisión antes del siguiente.

### 8.2 Backlog detallado (catálogo, sin orden acordado)

Los puntos de abajo son el detalle de lo que falta; la hoja de ruta de §8.1 decide el orden.

> **Componentes que faltan:** el 2026-10-20 se documentó por qué faltaban `Dock`, `Command`,
> `Settings`, `Tree`, `List`, `Carousel`, `Diff`, `Chart` y `Sidebar`, qué costaría cada uno y su
> impacto en host, protocolo y clientes: ver
> [GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md) (con el plan por tandas). `Tree`,
> `Diff` y `Carousel` (M2) y `Sidebar` y `Chart` (M3) ya están; quedan `Dock`, `Command` y
> `Settings` (M10) y `List`. `Editor` y `Speech` siguen fuera por dependencias.

- Componentes: **hecho el 2026-10-20** (HISTORIAL.md, "Accesibilidad y componentes de gpui-kit"):
  Basic (`Alert`, `Badge`, `Tag`, `Kbd`, `Progress`, `Spinner`, `Skeleton`, `Rating`,
  `Pagination`, `Collapsible`, `Accordion`, `Avatar`, `Stepper`), Form (`NumberInput`,
  `OtpInput`, `Form`, `Field`), Layout (`GroupBox`, `DescriptionList`, `StatusBar`, `Toolbar`,
  `Resizable`) y Advanced (`Calendar`), con `Accordion` y el redimensionado dentro de lo que el
  árbol puede expresar. Lo que queda se detalla en
  [GPUI_BRIDGE_COMPONENTS.md](GPUI_BRIDGE_COMPONENTS.md): `Tree`, `Diff` y `Carousel` (**hecho**,
  M2) y `Sidebar` y `Chart` (**hecho**, M3); `Dock`, `Command` y `Settings` en M10 (`Dock` con
  un ADR); `List` según necesidad. El editor de código (pide `tree-sitter`,
  que engorda el host), markdown (`TextView`) y el selector de color siguen pendientes.
- Ventanas: ocultar y mostrar ventanas sin cerrarlas, posición inicial y tamaño mínimo, eventos de foco y redimensionado de ventana. **Hecho el 2026-10-21** (M1, HISTORIAL.md, «Fase 28»).
- Rendimiento: redibujar solo los nodos que cambiaron (atajo de la fase 22). Medido el 2026-10-19 (HISTORIAL.md, "Medición"): el transporte no es el cuello de botella, el host aplica ~25 µs por nodo y cualquier animación satura un núcleo; el `batch` de operaciones no hace falta. **Hecho el 2026-10-20** (HISTORIAL.md, "Extra — Rendimiento: redibujado incremental"): un nodo con `animate-*` se dibuja en una vista propia, los cuadros van a lo sumo a 60 por segundo y el texto no se realoca por cuadro; la ventana en reposo sigue sin gastar CPU. Lo que queda: GPUI marca como sucios también los ancestros de la vista que se notifica, así que la ventana sigue recorriendo el árbol en cada cuadro (~54 ms por cuadro con 20 000 nodos en debug); con `VirtualList` el recorrido es de las filas visibles.
- Imágenes remotas: no se agrega HTTP al host (la decodificación pesa un orden de magnitud más que el transporte). El cliente baja el archivo y manda una ruta —lo más barato— o los bytes en una línea si hace falta. **Hecho el 2026-10-20** (HISTORIAL.md, "Extra — Imágenes por bytes"): `Image.data` acepta base64 estándar; el host lo decodifica una vez a un archivo por nodo (mismos formatos y animaciones que `src`) y lo borra con el nodo o con el árbol. `src` sigue siendo lo más barato para archivos locales.
- Accesibilidad: **hecho el 2026-10-20** (HISTORIAL.md, "Accesibilidad y componentes de gpui-kit"): todo nodo acepta `a11y` (`id`, `role`, `label`, `description`, `keyshortcuts` y el estado ARIA: `checked`, `selected`, `expanded`, `disabled`, `value`, `min`/`max`/`step`, `level`, `orientation`), validado y aplicado a la tecnología asistiva; la raíz de la ventana se informa como `window` con su título, que era el aviso de GPUI. Lo que queda: un repaso de lo que anuncia un lector de pantalla real (no hay uno en el entorno de pruebas) y roles automáticos más finos en los componentes que no dejan ponerlos desde fuera.
- Distribución: etiqueta de plataforma del wheel (hoy queda `py3-none-any` aunque lleva un binario de Windows) y publicación en npm/PyPI (solo con visto bueno: es un recurso remoto).
- Plataformas: compilar y probar en macOS y Linux.
