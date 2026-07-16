# Configuración

orbit usa dos niveles de configuración: la **configuración de usuario** global en `~/.config/orbit/config.toml` y los **archivos de scope** (`orbit.json`) en cada nivel del workspace.

---

## Configuración de usuario

**Ruta:** `~/.config/orbit/config.toml`

Contiene ajustes globales del binario: engine por defecto, directorio de instalación, comportamiento de auto-update.

### Claves principales

| Clave | Tipo | Descripción |
|---|---|---|
| `engine.default` | string | Engine por defecto al omitir `-e` (`opencode`, `gemini`, `claude`, `copilot`) |
| `engine.default_tenant` | string | Tenant por defecto en modo global |
| `engine.default_workspace` | string | Workspace por defecto |
| `install.dir` | path | Directorio de instalación del binario orbit |
| `update.auto_update_governance` | bool | Actualiza el repo de governance en background (default: true) |
| `update.auto_update_binary` | bool | Actualiza el binario orbit en background (default: true) |
| `update.check_on_startup` | bool | Verifica si hay nueva versión al iniciar (default: true) |

### Leer y escribir config

```bash
orbit config get engine.default
orbit config set engine.default claude
orbit config list               # muestra todos los valores activos
orbit config edit               # abre el archivo en $EDITOR
```

---

## Archivos de scope (`orbit.json`)

Cada nivel del workspace puede tener un archivo de configuración propio. El formato preferido es `orbit.json` o `orbit.jsonc` (con comentarios). También se acepta el formato legado `opencode.json`.

### Prioridad de archivos (de mayor a menor)

1. `orbit.json`
2. `orbit.jsonc`
3. `opencode.json`
4. `opencode.jsonc`

Solo se carga el primer archivo encontrado en cada scope.

### Formato `orbit.json`

```jsonc
{
  // Instrucciones adicionales para el engine
  "instructions": [
    "./context.md",
    "./source-of-truth/README.md"
  ],

  // Variables de entorno inyectadas al lanzar
  "env": {
    "MY_VAR": "valor",
    "SECRET_VAR": "secret://keychain/MY_SECRET"
  },

  // Agentes personalizados (solo para engines que los soporten)
  "agents": [
    {
      "name": "build",
      "description": "Build agent",
      "instructions": "./agents/build.md"
    }
  ]
}
```

### Variables de entorno en `orbit.json`

Las variables se inyectan en el entorno del engine al lanzar. Para secretos, usa los prefijos de resolver:

| Prefijo | Descripción |
|---|---|
| `secret://keychain/<key>` | Lee del keychain del sistema operativo |
| `env://<VAR>` | Referencia otra variable de entorno |
| `file://<path>` | Lee el valor de un archivo |

---

## Servidores MCP por scope

Cada scope puede tener su propio `mcp.json`. Ver [Servidores MCP](Servidores-MCP) para el formato y uso.

---

## Variables de entorno del sistema

orbit exporta estas variables al lanzar un engine:

| Variable | Descripción |
|---|---|
| `AI_ENGINE` | Engine activo |
| `AI_WORKSPACE_ROOT` | Ruta al workspace de código |
| `AI_CONTEXT_ROOT` | AI root del workspace |
| `AI_GLOBAL_ROOT` | AI root global (`~/AI`) |
| `AI_TENANT` | Tenant activo |
| `AI_PROJECT` | Proyecto activo |
| `AI_REPOSITORY` | Repositorio activo |
| `AI_GLOBAL_MODE` | `true` si se lanzó en modo global |
| `ORBIT_CONFIG_HOME` | Directorio XDG real de config (preservado antes del override) |
| `XDG_CONFIG_HOME` | Sobreescrito al directorio de runtime para aislamiento de sesión |

---

## Inspeccionar config activa

```bash
orbit launch . --dry-run    # reporte visual de todas las capas de config
orbit context               # instrucciones y MCP activos
orbit config list           # valores de usuario config
orbit status                # snapshot rápido
```

El reporte `--dry-run` muestra cada capa con:
- `✓` — archivo encontrado y cargado
- `·` — scope existe pero sin archivo de config
- Las entradas globales se etiquetan como `(global)`, las del workspace como `(workspace)`
