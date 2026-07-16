# Comandos

Referencia completa de todos los comandos de orbit, organizados por categoría.

---

## Setup y configuración inicial

### `orbit setup`
Wizard de primera configuración: engines, plugins, directorio de instalación.
```bash
orbit setup
orbit setup --no-install    # omite la instalación de engines
```

### `orbit init`
Inicializa el AI root clonando un repo de governance o creando uno local.
```bash
orbit init <url>            # clona un repositorio de governance
orbit init --scaffold       # crea un AI root local sin repo remoto
```

### `orbit doctor`
Diagnóstico completo del entorno. Verifica tmux, engines, AI root, daemon, instalación, plugins.
```bash
orbit doctor
```

---

## Lanzamiento y sesiones

### `orbit launch`
Lanza un engine con el contexto completo del scope.
```bash
orbit launch .                              # auto-detecta scope desde cwd
orbit launch WORKSPACE TENANT
orbit launch WORKSPACE TENANT PROJECT REPO
orbit launch -e claude .                    # engine explícito
orbit launch . --dry-run                    # inspecciona capas sin lanzar
orbit launch . --no-tmux                    # sin sesión tmux
```

### `orbit session`
Gestiona sesiones tmux activas.
```bash
orbit session list
orbit session attach [nombre]
orbit session kill <nombre>
orbit session clean
```

### `orbit ls`
Navega la jerarquía workspace → tenant → proyecto → repositorio.
```bash
orbit ls
orbit ls WORKSPACE
orbit ls WORKSPACE TENANT
orbit ls WORKSPACE TENANT PROJECT
```

---

## Configuración

### `orbit config`
Lee y escribe valores de configuración con notación de puntos.
```bash
orbit config get engine.default
orbit config set engine.default claude
orbit config set engine.default_tenant MYCO
orbit config set install.dir ~/.local/bin
orbit config list                            # muestra toda la config activa
orbit config edit                            # abre en $EDITOR
```

### `orbit env`
Gestiona variables de entorno en `orbit.json` por scope.
```bash
orbit env list
orbit env get MY_VAR
orbit env set MY_VAR value
orbit env delete MY_VAR
```
Ver [Secretos y variables](Secretos-y-variables) para más detalles.

### `orbit secret`
Gestiona secretos en el keychain del sistema operativo.
```bash
orbit secret set MY_SECRET
orbit secret get MY_SECRET
orbit secret delete MY_SECRET
```
Ver [Secretos y variables](Secretos-y-variables) para más detalles.

### `orbit context`
Inspecciona las capas de contexto, instrucciones y MCP activos.
```bash
orbit context
```

### `orbit snapshot`
Sincroniza el context file generado por el engine al repositorio de governance.
```bash
orbit snapshot
orbit snapshot --file CLAUDE.md
orbit snapshot --stdin
orbit snapshot --dry-run
```

---

## Engines

### `orbit engines`
Gestiona el ciclo de vida de los engines.
```bash
orbit engines list
orbit engines install <name>
orbit engines update [name]       # uno o todos los instalados
orbit engines info <name>
```

### `orbit auth`
Detección y gestión de autenticación de engines.
```bash
orbit auth                        # estado de todos los engines
orbit auth <engine>               # proxea al flujo nativo del engine
orbit auth --check                # sale con código 1 si alguno no está configurado
```

---

## Plugins

### `orbit plugins`
Gestiona el ciclo de vida de plugins.
```bash
orbit plugins list
orbit plugins install <name>
orbit plugins enable <name>
orbit plugins disable <name>
orbit plugins info <name>
orbit plugins wrap <name>         # envuelve el engine activo con el plugin
orbit plugins unwrap <name>
orbit plugins run <name> <cmd>    # ejecuta un comando del plugin
```
Ver [Plugins](Plugins) para más detalles.

---

## Servidores MCP

### `orbit mcp`
Gestiona servidores MCP por scope.
```bash
orbit mcp list
orbit mcp enable <name>
orbit mcp disable <name>
orbit mcp info <name>
orbit mcp list --scope global     # scope explícito: global|tenant|project|repo
```
Ver [Servidores MCP](Servidores-MCP) para más detalles.

---

## Sistema de planes

### `orbit plan`
Crea y ejecuta planes de ejecución autónoma.
```bash
orbit plan create <descripción>
orbit plan list
orbit plan run <id>
orbit plan status <id>
orbit plan show <id>
```
Ver [Sistema de planes](Sistema-de-planes) para más detalles.

### `orbit memory`
Busca y gestiona el historial de ejecuciones de planes.
```bash
orbit memory search <query>
orbit memory list
orbit memory show <id>
```

---

## Red y sharing

### `orbit serve`
Comparte tu instancia orbit en la red local.
```bash
orbit serve                       # inicia en background (non-blocking por defecto)
orbit serve --foreground          # inicia en primer plano
orbit serve stop                  # detiene el servidor
orbit serve status                # estado del servidor
```
Ver [Compartir y descubrir](Compartir-y-descubrir) para más detalles.

### `orbit discover`
Encuentra instancias orbit en la red local.
```bash
orbit discover
```

---

## Sistema

### `orbit status`
Snapshot rápido del estado operativo (< 200ms).
```bash
orbit status                      # salida legible, máximo 8 líneas
orbit status --json               # salida JSON estructurada para scripts
```
Muestra: workspace, engine (instalación + auth), tenant, scope desde cwd, estado del daemon con conteo de sesiones, versión, modo activo.

### `orbit daemon`
Controla el proceso `orbitd` en background.
```bash
orbit daemon serve                # inicia el daemon
orbit daemon start
orbit daemon stop
orbit daemon status
```

### `orbit mode`
Cambia entre versiones del binario.
```bash
orbit mode stable                 # instala la última release estable
orbit mode dev [path]             # symlink a un build local
orbit mode beta                   # instala la última pre-release
orbit mode status                 # modo activo y detalles del binario
```

### `orbit update`
Sincroniza governance y actualiza el binario.
```bash
orbit update                      # actualiza governance + binario
orbit update --force              # reinstala aunque ya sea la última versión
```

### `orbit workspace`
Registra y gestiona configuraciones de múltiples workspaces.
```bash
orbit workspace list
orbit workspace add <nombre> <ruta>
orbit workspace remove <nombre>
```

### `orbit notify`
Envía notificaciones de escritorio.
```bash
orbit notify "mensaje"
```

### `orbit completions`
Genera scripts de autocompletado para la shell.
```bash
orbit completions bash
orbit completions zsh
orbit completions fish
```

### `orbit man`
Genera e instala páginas man.
```bash
orbit man generate
orbit man install
```

### `orbit jira`
Integración con tableros Jira.
```bash
orbit jira list
orbit jira show <ticket>
```

---

## Atajos

| Atajo | Equivalente |
|---|---|
| `orbit .` | `orbit launch .` |
| `orbit` (sin args) | Abre el TUI |
