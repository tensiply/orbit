# Servidores MCP

Los servidores MCP (Model Context Protocol) amplían las capacidades del engine con herramientas externas. orbit gestiona MCPs en múltiples capas que se acumulan por scope.

---

## Cómo se cargan los MCPs

El orden de carga (de menor a mayor prioridad):

| # | Capa | Archivo |
|---|---|---|
| 1 | Plugins | `~/.orbit/plugins.mcp.json` (generado por `orbit plugins enable`) |
| 2 | Global | `~/AI/mcp.json` |
| 3 | Workspace | `~/WORKSPACE/AI/mcp.json` |
| 4 | Tenant | `…/tenants/TENANT/mcp.json` |
| 5 | Proyecto | `…/projects/PROJECT/mcp.json` |
| 6 | Repositorio | `…/repositories/REPO/mcp.json` |

Los MCPs de scopes más específicos pueden sobrescribir los de scopes más generales. Todos los MCPs habilitados se ensamblan y pasan al engine al lanzar.

Los `mcpServers` definidos directamente en `orbit.json` de cualquier scope también se cargan — funcionan para **todos los engines**, no solo OpenCode.

---

## `orbit mcp`

```bash
orbit mcp list                      # lista MCPs del catálogo con estado (habilitado/deshabilitado)
orbit mcp enable <name>             # habilita un MCP (pide las variables requeridas)
orbit mcp disable <name>            # deshabilita un MCP
orbit mcp info <name>               # descripción, variables, estado por capa
```

### Scope

Por defecto, el scope se auto-detecta desde el directorio actual. Para especificarlo explícitamente:

```bash
orbit mcp list --scope global
orbit mcp enable mi-servidor --scope workspace
orbit mcp enable mi-servidor --scope tenant
orbit mcp enable mi-servidor --scope project
orbit mcp enable mi-servidor --scope repo
```

Los niveles válidos son: `global`, `workspace`, `tenant`, `project`, `repo`.

### MCPs custom (no en el catálogo)

`orbit mcp enable` acepta cualquier MCP, no solo los del catálogo integrado. Para MCPs propios o de terceros, pásalos con la definición inline o habilítalos directamente:

```bash
orbit mcp enable mi-servidor-custom --scope workspace
```

Si el MCP no está en el catálogo, orbit pedirá los parámetros necesarios interactivamente.

---

## Catálogo de MCPs

orbit incluye un catálogo de MCPs conocidos compilado en el binario (`config/catalog/mcps.toml`). El catálogo se actualiza en cada release.

```bash
orbit mcp list          # muestra catálogo completo con estado de habilitación
orbit mcp info <name>   # detalles, variables requeridas, estado en cada capa
```

Las variables marcadas como secretas en el catálogo muestran un hint para usar resolvers de keychain en lugar de guardar el valor en claro.

---

## Formato `mcp.json`

```jsonc
{
  "mcpServers": {
    "mi-servidor": {
      "command": "npx",
      "args": ["-y", "@mi-org/mi-mcp@latest"],
      "env": {
        "MI_API_KEY": "keychain://MI_KEY"
      }
    },
    "mi-servidor-local": {
      "command": "/usr/local/bin/mi-mcp",
      "args": ["--port", "3000"]
    }
  }
}
```

El mismo formato funciona dentro del bloque `mcpServers` de `orbit.json`.

### Variables de entorno en MCPs

Puedes usar los mismos resolvers que en `orbit.json`:

| Prefijo | Descripción |
|---|---|
| `keychain://<key>` | Lee del keychain del SO |
| `secret://keychain/<key>` | Alias legado de `keychain://` |
| `env://<VAR>` | Referencia una variable de entorno |
| `file://<path>` | Lee de un archivo |

---

## MCPs de plugins

Cuando habilitas un plugin con `orbit plugins enable <name>`, sus servidores MCP se registran automáticamente en `~/.orbit/plugins.mcp.json` y se cargan en todas las sesiones como capa base.

```bash
orbit plugins enable playwright     # registra @playwright/mcp en todas las sesiones
orbit plugins disable playwright    # elimina el MCP de las sesiones
```

---

## Inspeccionar MCPs activos

```bash
orbit context show          # MCPs activos con atribución de capa
orbit context --dry-run     # muestra de qué capa (global, workspace, tenant:X, plugins...) viene cada MCP
orbit launch . --dry-run    # reporte completo incluyendo MCPs
orbit mcp list              # catálogo con estado de habilitación
```

`orbit context show` incluye una sección de MCP servers donde cada entrada indica su capa de origen, por ejemplo:

```
● github          npx @modelcontextprotocol/server-github    [workspace]
● playwright      npx @playwright/mcp@latest                 [plugins]
● mi-servidor     uvx mi-mcp-server                          [tenant:AIDEV]
```
