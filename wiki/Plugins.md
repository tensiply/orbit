# Plugins

Los plugins son herramientas opcionales con su propio ciclo de vida de instalación. Pueden registrar servidores MCP, envolver engines y agregar contexto a las sesiones.

---

## Plugins built-in

| Plugin | Categoría | Descripción |
|---|---|---|
| `headroom` | ai | Capa de compresión de contexto (60–95% menos tokens). Puede envolver el engine activo |
| `playwright` | dev | Automatización de browser vía `@playwright/mcp` |
| `rust-analyzer` | dev | Servidor de lenguaje Rust. Instala via `rustup component add` o `cargo install` |
| `markitdown` | productivity | Convierte PDF, DOCX, XLSX, imágenes a Markdown via Python venv gestionado por orbit |
| `linear` | productivity | Linear.app: issues, proyectos, ciclos (requiere OAuth2.1) |
| `jenkins` | infra | Jenkins CI/CD: builds, pipelines, logs de ejecución |
| `sonarcloud` | infra | SonarCloud: quality gates, issues, security hotspots, cobertura |
| `gcloud` | infra | Google Cloud: autenticación aislada por scope, Cloud Run, GKE |
| `aws` | infra | AWS: autenticación por scope, S3, Lambda, EC2 |
| `kubectl` | infra | Kubernetes: contextos aislados por scope |
| `jira` | productivity | Integración con Jira |
| `cargo` | dev | Automatización de build con Cargo |
| `make` | dev | Integración con Makefile |
| `npm` | dev | Integración con Node.js package manager |
| `pytest` | test | Runner de tests Python |
| `ftp` / `sftp` | infra | Transferencia de archivos vía MCP stdio |

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

Al habilitar un plugin, orbit escribe sus servidores MCP en `~/.orbit/plugins.mcp.json`. Esta capa se carga como base en toda sesión orbit — antes que los MCPs de scope. Los `mcp.json` de cada scope pueden sobrescribir los MCPs de plugins.

El estado de habilitación se persiste en `~/.orbit/plugin-state.toml`.

### Enable/disable por scope

Por defecto `orbit plugins enable` escribe en el scope actual (repo, proyecto, tenant o global según dónde estés). Puedes especificar el scope explícitamente:

```bash
orbit plugins enable sonarcloud --scope global     # activa para todas las sesiones
orbit plugins enable linear --scope tenant         # solo para el tenant actual
```

---

## Plugins con wrap

Algunos plugins, como `headroom`, pueden envolver el engine activo:

```bash
orbit plugins wrap headroom         # lanza el engine a través de headroom (compresión de contexto)
orbit plugins unwrap headroom       # vuelve al engine directo
```

El wrap modifica el comando de lanzamiento — en lugar de ejecutar `claude` directamente, ejecuta `headroom` que a su vez llama a `claude`.

---

## Autenticación OAuth2.1 PKCE

Algunos plugins requieren autenticación con un servicio externo. orbit implementa el flujo OAuth2.1 con PKCE completo:

```bash
orbit plugins auth linear      # abre el navegador → autoriza → guarda token en keychain
```

El token se renueva automáticamente cuando expira. Puedes gestionar credenciales manualmente:

```bash
orbit secret get linear-token
orbit secret set linear-token TOKEN
```

Plugins que usan OAuth2.1: `linear`. Plugins que usan credenciales estáticas (API key): `sonarcloud`, `jenkins`.

---

## Plugins custom

Puedes agregar plugins propios colocando archivos `.toml` en `~/.orbit/plugins/` sin necesidad de recompilar orbit.

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
