# Arquitectura

orbit es un workspace Cargo con múltiples crates. El binario principal (`orbit`) se comunica con un daemon persistente (`orbitd`) vía Unix socket.

---

## Diagrama general

```
┌─────────────────────────────────────────────────┐
│                  orbit (binario)                │
│                                                 │
│  ┌──────────────┐        ┌───────────────────┐  │
│  │  CLI (clap)  │        │   TUI (ratatui)   │  │
│  │  subcomandos │        │   modo interactivo│  │
│  └──────┬───────┘        └─────────┬─────────┘  │
│         └──────────────────────────┘            │
│                      │                          │
│              ┌────────┴────────┐                │
│              │  IPC Client     │                │
│              │  (Unix socket)  │                │
│              └────────┬────────┘                │
└───────────────────────┼─────────────────────────┘
                        │ ~/.local/share/orbit/orbit.sock
┌───────────────────────┴─────────────────────────┐
│                  orbitd (daemon)                │
│                                                 │
│  ┌───────────────┐   ┌───────────────────────┐  │
│  │ Context Cache │   │ Session Manager       │  │
│  └───────────────┘   └───────────────────────┘  │
│  ┌───────────────┐   ┌───────────────────────┐  │
│  │ Engine Runner │   │ Hub Connector         │  │
│  └───────────────┘   └───────────────────────┘  │
└─────────────────────────────────────────────────┘
```

El cliente auto-arranca el daemon si no está corriendo.

---

## Estructura de crates

```
orbit/
├── bins/
│   ├── orbit/              ← Binario estable
│   └── orbit-dev/          ← Binario de desarrollo (verbose, acceso directo a internals)
├── crates/
│   ├── orbit-core/         ← Tipos compartidos, config, contexto, errores
│   ├── orbit-cli/          ← Entry point + definición de subcomandos (clap)
│   ├── orbit-tui/          ← Componentes TUI (ratatui)
│   ├── orbit-daemon/       ← Servidor daemon (tokio, IPC server)
│   ├── orbit-client/       ← Cliente IPC (usado desde CLI y TUI)
│   ├── orbit-engine/       ← Lanzamiento de engines, merge de config, resolver de scope
│   ├── orbit-planner/      ← Motor de planes: intent parsing, policy, templates
│   ├── orbit-eval/         ← Evaluación de planes en CI sin ejecutar engines
│   └── orbit-integration-tests/
├── plugins/                ← Definiciones de plugins built-in (.toml)
├── config/catalog/         ← Catálogo de engines y MCPs (engines.toml, mcps.toml)
└── tests/                  ← Integration tests
```

---

## Responsabilidades de cada crate

| Crate | Responsabilidad | Regla |
|---|---|---|
| `orbit-core` | Tipos compartidos, structs de config, contexto, errores | Sin efectos de I/O en funciones puras |
| `orbit-engine` | Lógica de lanzamiento, merge de config, materialización de agentes | Puede hacer I/O; sin CLI parsing |
| `orbit-cli` | Definición de comandos clap, dispatch de subcomandos | Delega toda la lógica a engine/core |
| `orbit-daemon` | Servidor IPC async, gestor de sesiones | Solo tokio; aislado del código síncrono |
| `orbit-client` | Cliente IPC ligero | Sin lógica de negocio |
| `orbit-tui` | Componentes ratatui, vistas | Sin acceso directo a config |
| `orbit-planner` | Motor de planes, evaluación de intenciones, templates | |
| `orbit-eval` | Validación estructural de planes para CI | |

---

## Protocolo IPC

- **Transporte:** Unix domain socket en `~/.local/share/orbit/orbit.sock`
- **Formato:** JSON-RPC 2.0 sobre socket
- **Auto-start:** si el cliente no detecta el socket, arranca `orbitd` y espera a que esté listo

---

## Catálogo compilado

Los catálogos de engines y MCPs se compilan en el binario en tiempo de build desde `config/catalog/engines.toml` y `config/catalog/mcps.toml`. No hay fetching dinámico — se actualiza con cada release.

---

## Variables de entorno del engine

Antes de ejecutar el engine, orbit llama a `set_env()` en `orbit-engine/src/launcher/mod.rs`. Este es el único lugar donde se llama `std::env::set_var` — nunca en otro lugar del código.

---

## Stack de dependencias principales

| Crate | Propósito |
|---|---|
| `clap` | CLI parsing con derive macros |
| `ratatui` + `crossterm` | TUI — componentes, layout, eventos de teclado |
| `tokio` | Runtime async para el daemon |
| `serde` + `serde_json` | Serialización de config y protocolo IPC |
| `anyhow` + `thiserror` | Error handling en boundaries public/internal |
| `tracing` + `tracing-subscriber` | Logging estructurado |
| `directories` | XDG dirs cross-platform |
| `notify` | Watcher de archivos para hot-reload de contexto |

---

## Binarios

| Binario | Propósito | Distribución |
|---|---|---|
| `orbit` | Release estable — para uso diario | crates.io / releases |
| `orbit-dev` | Build de desarrollo — logging verbose, acceso directo a internals | Solo local |

`orbit-dev` comparte todos los crates con `orbit`. La diferencia es el acceso directo a `orbit-engine` y `orbit-daemon` sin pasar por IPC, útil para probar funcionalidades antes de exponerlas en la API estable.
