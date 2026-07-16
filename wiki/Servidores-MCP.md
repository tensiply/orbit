# Servidores MCP

Los servidores MCP (Model Context Protocol) amplían las capacidades del engine con herramientas externas. orbit gestiona MCPs en múltiples capas que se acumulan por scope.

---

## Cómo se cargan los MCPs

El orden de carga (de menor a mayor prioridad):

1. MCPs de plugins habilitados (`~/.config/orbit/plugins.mcp.json`)
2. Global (`~/AI/mcp.json`)
3. Workspace (`~/WORKSPACE/AI/mcp.json`)
4. Tenant
5. Proyecto
6. Repositorio

Los MCPs de scopes más específicos pueden sobrescribir los de scopes más generales. Todos los MCPs habilitados se ensamblan y pasan al engine al lanzar.

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
orbit mcp enable mi-servidor --scope tenant
orbit mcp enable mi-servidor --scope project
orbit mcp enable mi-servidor --scope repo
```

---

## Catálogo de MCPs

orbit incluye un catálogo de MCPs conocidos compilado en el binario (`config/catalog/mcps.toml`). El catálogo se actualiza en cada release.

```bash
orbit mcp list          # muestra catálogo completo con estado de habilitación
orbit mcp info <name>   # detalles, variables requeridas, estado en cada capa
```

Las variables marcadas como secretas en el catálogo muestran un hint para usar variables de entorno en lugar de guardar el valor en claro.

---

## Formato `mcp.json`

```jsonc
{
  "mcpServers": {
    "mi-servidor": {
      "command": "npx",
      "args": ["-y", "@mi-org/mi-mcp@latest"],
      "env": {
        "MI_API_KEY": "secret://keychain/MI_KEY"
      }
    },
    "mi-servidor-local": {
      "command": "/usr/local/bin/mi-mcp",
      "args": ["--port", "3000"]
    }
  }
}
```

### Variables de entorno en MCPs

Puedes usar los mismos resolvers que en `orbit.json`:

| Prefijo | Descripción |
|---|---|
| `secret://keychain/<key>` | Lee del keychain del SO |
| `env://<VAR>` | Referencia una variable de entorno |
| `file://<path>` | Lee de un archivo |

---

## MCPs de plugins

Cuando habilitas un plugin con `orbit plugins enable <name>`, sus servidores MCP se registran automáticamente en `~/.config/orbit/plugins.mcp.json` y se cargan en todas las sesiones como capa base.

```bash
orbit plugins enable playwright     # registra @playwright/mcp en todas las sesiones
orbit plugins disable playwright    # elimina el MCP de las sesiones
```

---

## Inspeccionar MCPs activos

```bash
orbit launch . --dry-run    # muestra todos los MCPs que se cargarían, por capa
orbit context               # MCPs activos en el scope actual
orbit mcp list              # catálogo con estado de habilitación
```
