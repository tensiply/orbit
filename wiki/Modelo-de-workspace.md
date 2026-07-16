# Modelo de workspace

orbit organiza el contexto en cinco niveles de scope que se acumulan de global a específico: **global → workspace → tenant → proyecto → repositorio**.

---

## Estructura de directorios

```
~/AI/                              ← raíz global (siempre cargada)
└── tenants/
    └── MYCO/                      ← tenant
        └── projects/
            └── backend/           ← proyecto
                └── repositories/
                    └── api/       ← repositorio

~/MYCO/                            ← código real (separado del contexto)
    └── backend/
        └── api/
```

El directorio `~/AI` es un repositorio de governance — contiene instrucciones compartidas, definiciones de agentes y servidores MCP para todos tus workspaces. Siempre se carga sin importar desde qué workspace lances.

### Multi-workspace

Si trabajas con múltiples workspaces (por ejemplo `~/AI` personal y `~/BeFra/AI` laboral), orbit soporta un AI root global y un AI root de workspace independientes:

```
~/AI/                              ← raíz global (siempre cargada)
~/BeFra/AI/                        ← raíz del workspace BeFra
    └── tenants/
        └── DEVTEAM/
```

---

## Qué puede definir cada nivel

| Nivel | Config | Instrucciones | Servidores MCP |
|---|---|---|---|
| Global (`~/AI`) | ✓ | ✓ | ✓ |
| Workspace (`~/WORKSPACE/AI`) | ✓ | ✓ | ✓ |
| Tenant | ✓ | ✓ | ✓ |
| Proyecto | ✓ | ✓ | ✓ |
| Repositorio | ✓ | ✓ | ✓ |

orbit funde todo en una sola configuración antes de lanzar el engine. Los niveles más específicos sobrescriben a los más generales.

---

## Archivos de configuración por nivel

Dentro de cada directorio de scope puedes crear:

| Archivo | Propósito |
|---|---|
| `orbit.json` / `orbit.jsonc` | Config del scope (preferido sobre `opencode.json`) |
| `opencode.json` / `opencode.jsonc` | Config legada (compatible) |
| `mcp.json` | Servidores MCP del scope |
| `context.md` / `CLAUDE.md` / `AGENTS.md` | Instrucciones para el engine |

---

## Resolución de scope

### Desde el directorio actual

```bash
orbit launch .          # auto-detecta scope desde cwd
orbit .                 # alias
```

orbit camina los directorios ancestros para encontrar la raíz del workspace y mapea los segmentos del path a tenant / proyecto / repositorio.

### Explícito

```bash
orbit launch TENANT                         # solo tenant
orbit launch WORKSPACE TENANT               # workspace + tenant
orbit launch WORKSPACE TENANT PROJECT REPO  # scope completo
```

La resolución es **case-insensitive** — `myco` y `MYCO` resuelven al mismo directorio.

---

## Orden de carga

Cuando orbit lanza una sesión, carga las capas en este orden:

1. `~/AI` (raíz global)
2. `~/WORKSPACE/AI` (raíz del workspace, si es diferente de global)
3. Tenant (dentro del AI root correspondiente)
4. Proyecto
5. Repositorio

Los MCPs se acumulan de todas las capas. Las instrucciones se concatenan. Los valores de config se sobrescriben de menor a mayor especificidad.

---

## Inicializar el AI root

```bash
# Clonar un repositorio de governance existente
orbit init <url-del-repo>

# Crear un AI root local sin repositorio remoto
orbit init --scaffold
```

---

## Inspeccionar capas activas

```bash
orbit launch . --dry-run    # muestra todas las capas, config, MCP e instrucciones sin lanzar
orbit context               # inspecciona capas de contexto, instrucciones y MCP
```

El reporte `--dry-run` muestra cada capa con su estado (`✓` = archivo encontrado, `·` = no existe).

---

## Sincronizar contexto

```bash
orbit snapshot    # sincroniza el context.md generado por el engine al repo de governance
```

Útil después de ejecutar `/init` dentro del engine — copia el archivo resultante (`CLAUDE.md`, `AGENTS.md`, etc.) al `source-of-truth/context.md` del scope correspondiente.
