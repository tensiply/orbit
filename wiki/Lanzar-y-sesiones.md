# Lanzar y sesiones

## `orbit launch`

El comando principal. Resuelve el scope, carga todo el contexto y lanza el engine en una sesión tmux.

```bash
orbit launch .                              # auto-detecta scope desde cwd
orbit launch WORKSPACE TENANT              # scope explícito
orbit launch WORKSPACE TENANT PROJECT REPO # scope completo
orbit .                                    # alias de orbit launch .
```

### Flags

| Flag | Descripción |
|---|---|
| `-e, --engine <name>` | Engine a usar (`opencode`, `gemini`, `claude`, `copilot`). Si se omite, usa `engine.default` de config |
| `--dry-run` | Muestra el reporte de capas sin lanzar el engine |
| `--no-tmux` | Lanza el engine directamente sin crear una sesión tmux |
| `--no-update` | Omite el auto-update de governance y binario para esta invocación |

### Ejemplos

```bash
orbit launch .                          # desde ~/MYCO/backend/api/ detecta el scope completo
orbit launch -e claude .                # usa Claude Code como engine
orbit launch . --dry-run                # inspecciona config sin lanzar
orbit launch MYCO backend api           # scope explícito sin detección
```

### ¿Qué hace orbit launch?

1. Resuelve el scope (desde cwd o argumentos)
2. Carga las capas de config: global → workspace → tenant → proyecto → repo
3. Ensambla todos los servidores MCP de todas las capas
4. Concatena las instrucciones de todas las capas
5. Establece el título del terminal (xterm OSC) y el nombre de la ventana tmux
6. Lanza el engine con el contexto combinado

---

## `orbit session`

Gestiona las sesiones tmux activas.

```bash
orbit session list              # lista sesiones activas
orbit session attach [nombre]   # attach a una sesión (selector si hay varias)
orbit session kill <nombre>     # termina una sesión
orbit session clean             # elimina entradas de sesiones muertas
```

### `orbit session attach`

- Si solo hay una sesión activa, hace attach directamente
- Si hay múltiples, muestra un selector interactivo
- Usa `switch-client` cuando ya estás dentro de tmux; `attach-session` en caso contrario
- Verifica que la ventana tmux sigue existiendo antes de hacer attach

---

## TUI

Ejecuta `orbit` sin argumentos para abrir el TUI:

```
orbit
```

El tab principal es **Chat (Tab 0)**: escribe un objetivo en lenguaje natural y orbit genera y ejecuta un plan de IA. Los demás tabs dan visibilidad y control sobre el resto del sistema.

### Tabs del TUI

| Tab | Tecla | Nombre | Descripción |
|---|---|---|---|
| 0 | `0` | Chat | Interfaz de chat con el planner de IA — entrada principal |
| 1 | `1` | Sessions | Sesiones activas por scope — attach/kill |
| 2 | `2` | Launch | Formulario interactivo para lanzar sesiones |
| 3 | `3` | System | Estado del daemon y MCP servers por scope |
| 4 | `4` | Plans | Historial y estado de planes de IA |
| 5 | `5` | Engines | Engines instalados, versiones y estado de auth |
| 6 | `6` | Plugins | Plugins habilitados y su estado |
| 7 | `7` | Activity | Log de actividad reciente de sesiones |
| 8 | `8` | Memory | Entradas de memoria persistente por scope |
| 9 | `9` | Peers | Instancias orbit en la red local (requiere `orbit serve`) |

### Atajos de teclado

| Tecla | Acción |
|---|---|
| `0`–`9` | Ir al tab correspondiente |
| `Tab` / `Shift+Tab` | Ciclar entre tabs |
| `↑↓` | Navegar lista |
| `↵` / `a` | Attach a sesión (tab Sessions) |
| `k` | Kill sesión |
| `w` | Cambiar workspace |
| `←→` | Ciclar workspaces (en tab Launch) |
| `↓` | Abrir dropdown (Tenant/Project/Repo en Launch) |
| `q` / `Esc` | Salir del TUI |

### Workspace selector

En el tab Launch, el selector de workspace detecta automáticamente los workspaces disponibles escaneando `~/` en busca de directorios con `orbit.toml` o `tenants/`. Cambiar de workspace recarga las opciones de tenant/proyecto/repositorio.

---

## Integración con tmux

orbit gestiona sesiones tmux con nombres descriptivos. Cuando lanza una sesión:
- Crea una nueva ventana tmux con nombre `orbit · <engine> · <tenant>/<project>/<repo>`
- Establece el título del terminal vía escape OSC de xterm
- Si ya estás dentro de tmux, usa `switch-client` en lugar de `attach-session`

Si tmux no está disponible, orbit puede lanzar el engine directamente con `--no-tmux`.

---

## Modo global

Si no se especifica scope, orbit lanza en modo global usando solo las capas base:

```bash
orbit launch              # modo global: solo carga ~/AI
orbit launch -e gemini    # modo global con engine específico
```
