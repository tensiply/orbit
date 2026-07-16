# Plugins

Los plugins son herramientas opcionales con su propio ciclo de vida de instalación. Pueden registrar servidores MCP, envolver engines y agregar contexto a las sesiones.

---

## Plugins built-in

| Plugin | Descripción |
|---|---|
| `headroom` | Capa de compresión de contexto (60–95% menos tokens). Puede envolver el engine activo |
| `playwright` | Automatización de browser vía `@playwright/mcp`. Registra un servidor MCP |
| `rust-analyzer` | Servidor de lenguaje Rust. Instala via `rustup component add` o `cargo install` |
| `cargo` | Automatización de build con Cargo |
| `make` | Integración con Makefile |
| `npm` | Integración con Node.js package manager |
| `pytest` | Runner de tests Python |
| `jira` | Integración con Jira |

---

## Comandos

```bash
orbit plugins list                  # lista todos los plugins con estado (instalado/habilitado)
orbit plugins install <name>        # instala un plugin (muestra métodos disponibles)
orbit plugins enable <name>         # activa el plugin (registra sus MCP servers)
orbit plugins disable <name>        # desactiva el plugin
orbit plugins info <name>           # descripción, métodos de instalación, estado
orbit plugins wrap <name>           # envuelve el engine activo con el plugin
orbit plugins unwrap <name>         # deshace el wrap
orbit plugins run <name> <cmd>      # ejecuta un comando del plugin
```

---

## Habilitar un plugin

```bash
orbit plugins install playwright    # instala playwright MCP
orbit plugins enable playwright     # registra el MCP en todas las sesiones
```

Al habilitar un plugin, orbit escribe sus servidores MCP en `~/.config/orbit/plugins.mcp.json`. Esta capa se carga como base en toda sesión orbit — antes que los MCPs de scope. Los `mcp.json` de cada scope pueden sobrescribir los MCPs de plugins.

El estado de habilitación se persiste en `~/.config/orbit/plugin-state.toml`.

---

## Plugins con wrap

Algunos plugins, como `headroom`, pueden envolver el engine activo:

```bash
orbit plugins wrap headroom         # lanza el engine a través de headroom (compresión de contexto)
orbit plugins unwrap headroom       # vuelve al engine directo
```

El wrap modifica el comando de lanzamiento — en lugar de ejecutar `claude` directamente, ejecuta `headroom` que a su vez llama a `claude`.

---

## Plugins custom

Puedes agregar plugins propios colocando archivos `.toml` en `~/.config/orbit/plugins/` sin necesidad de recompilar orbit.

### Formato de plugin

```toml
name = "mi-plugin"
description = "Mi herramienta personalizada"
version = "1.0.0"

[[install_methods]]
method = "npm"
package = "mi-paquete"

[[install_methods]]
method = "cargo"
crate = "mi-crate"

[[mcp_servers]]
name = "mi-mcp"
command = ["npx", "-y", "mi-paquete@latest"]
```

### Métodos de instalación soportados

| Método | Descripción |
|---|---|
| `npm` | `npm install -g <package>` |
| `cargo` | `cargo install <crate>` |
| `pip` | `pip install <package>` |
| `brew` | `brew install <formula>` |
| `rustup` | `rustup component add <component>` |
| `custom` | Comando arbitrario vía `install_cmd` |

---

## Integración con `orbit doctor` y `orbit setup`

`orbit doctor` muestra una sección de plugins con el estado de cada uno.
`orbit setup` ofrece instalar plugins interactivamente durante la configuración inicial.
