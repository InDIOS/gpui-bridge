> **Estado: aplicada.** Esta recomendación se implementó en la rama `dev`, con correcciones
> (pnpm usa `pnpm-workspace.yaml`, el `[profile.release]` subió a la raíz de Cargo, el host se
> compila una vez y se *stagea* en cada paquete, y los clientes buscan el host subiendo por el
> árbol). Las decisiones y el porqué están en [`adr/0001-monorepo.md`](adr/0001-monorepo.md); el
> árbol vigente, en [`GPUI_BRIDGE_PLAN.md`](GPUI_BRIDGE_PLAN.md) §4.

Para un proyecto multi-lenguaje (portar GPUI + GPUI Kit) que actúa como un puente (bridge) y tiene múltiples clientes, la mejor práctica actual es usar una arquitectura **Monorepo**. Estructura que se propone (separar los paquetes distribuibles de la documentación, ejemplos y pruebas). 

Estructura limpia, escalable y estándar para este caso:

```text
gpui-bridge/
├── .github/                     # (Recomendado) Workflows de CI/CD para probar todos los lenguajes
├── docs/                        # Documentación central
│   ├── GPUI_BRIDGE_COMPONENTS.md
│   ├── GPUI_BRIDGE_PLAN.md
│   ├── HISTORIAL.md
│   └── PROTOCOL.md
├── examples/                    # Proyectos de ejemplo (usan los paquetes, no se publican)
├── conformance/                 # Suite de pruebas para asegurar que todos los lenguajes se comporten igual
├── benchmarks/                  # (Renombrado de 'bench') Pruebas de rendimiento cruzadas
├── packages/                    # 🌟 Aquí van todos los subproyectos distribuibles
│   ├── host/                    # El núcleo del bridge (probablemente en Rust/GPUI)
│   ├── client-node/             # SDK/Bindings para Node.js
│   ├── client-python/           # SDK/Bindings para Python
│   └── client-react/            # Wrapper/Componentes para React
├── .gitignore
├── README.md                    # El punto de entrada al proyecto
├── Cargo.toml                   # (Si 'host' es Rust) Configuración del Workspace de Cargo
├── package.json                 # (Si usas Node/React) Configuración del Workspace de NPM/pnpm/Yarn
└── LICENSE
```

### ¿Por qué esta estructura es la ideal para tu caso?

1. **`packages/` centralizado**: Al tener `host`, `client-node`, `client-python` y `client-react` aquí, dejas claro cuáles son los artefactos que se publican (en npm, PyPI, crates.io, etc.). Además, herramientas como Nx, Turborepo, pnpm workspaces o Cargo Workspaces (si usas Rust) buscan por defecto esta carpeta.
2. **`docs/` separado**: Los archivos `.md` que tenías sueltos (excepto el `README.md` principal) saturan la raíz. Moverlos a `docs/` mantiene la raíz limpia. El `README.md` debe quedarse en la raíz para que GitHub lo muestre por defecto.
3. **`examples/` fuera de `packages/`**: Los ejemplos no deberían ser paquetes que se publiquen en un registro de paquetes. Al estar en la raíz, pueden consumir tus paquetes locales (usando *workspaces*) para probar que todo funcione antes de publicar.
4. **`conformance/` y `benchmarks/` a nivel raíz**: Son suites de pruebas transversales. `conformance` es vital aquí: al tener múltiples lenguajes (Python, Node, React), necesitas una carpeta que valide que el protocolo (`PROTOCOL.md`) se cumpla exactamente igual en todos los clientes.

### Consejos para la migración y el manejo del Monorepo:

* **Workspaces (Espacios de trabajo)**: 
  * Si el `host` está en **Rust**, define un `Cargo.toml` en la raíz con `[workspace] members = ["packages/host", ...]`.
  * Si los clientes son de **JS/TS** (Node y React), define un `package.json` en la raíz con `"workspaces": ["packages/client-node", "packages/client-react", "examples/*"]`.
* **Nombres de paquetes**: Asegúrate de que el nombre en el `package.json` o `pyproject.toml` de cada cliente sea consistente (ej. `@tu-org/gpui-bridge-node`, `gpui-bridge-python`, etc.).
* **Herramientas (Opcional)**: Si el proyecto crece mucho, considera usar **Nx** o **Turborepo** para manejar los pipelines de construcción y testing (por ejemplo, que al cambiar el `host`, se re-compilen y testeen automáticamente todos los clientes).

### Pasos rápidos para moverlo:
1. Crea las carpetas `packages` y `docs` en la raíz.
2. Mueve `client-node`, `client-python`, `client-react` y `host` dentro de `packages/`.
3. Mueve `GPUI_BRIDGE_COMPONENTS.md`, `GPUI_BRIDGE_PLAN.md`, `HISTORIAL.md` y `PROTOCOL.md` dentro de `docs/`.
4. Renombra `bench` a `benchmarks` (opcional, pero más claro).
5. Deja `examples` y `conformance` en la raíz.
6. Actualiza las rutas relativas en tus archivos de configuración y en los scripts de CI/CD.
