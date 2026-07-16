# Engines

orbit soporta cuatro engines de IA intercambiables. El engine activo se configura globalmente o se especifica por invocación con `-e`.

---

## Engines disponibles

| Engine | Binario | Instalación | Descripción |
|---|---|---|---|
| `opencode` | `opencode` | npm | Agentic coding engine — soporta múltiples proveedores de IA |
| `gemini` | `gemini` | npm | Google Gemini CLI — IA agentic con los modelos de Google |
| `claude` | `claude` | npm | Claude Code — CLI oficial de Anthropic para Claude |
| `copilot` | `gh` (extensión) | gh extension | GitHub Copilot en la terminal |

---

## Instalación

```bash
orbit engines list                  # lista todos los engines con estado
orbit engines install opencode      # instala un engine
orbit engines install claude
orbit engines update                # actualiza todos los engines instalados
orbit engines update gemini         # actualiza un engine específico
orbit engines info claude           # descripción, versión instalada, estado de auth
```

orbit detecta la versión instalada ejecutando `<bin> --version` y extrae el número de versión. La versión npm más reciente se cachea 24h en `~/.local/share/orbit/engine-versions/`.

---

## Autenticación

```bash
orbit auth                          # estado de autenticación de todos los engines
orbit auth claude                   # proxea al flujo nativo de autenticación del engine
orbit auth --check                  # sale con código 1 si algún engine no está configurado (útil en CI)
```

orbit detecta autenticación sin hacer llamadas de red — verifica variables de entorno y directorios de configuración:

| Engine | Variables de entorno | Directorios de config |
|---|---|---|
| `opencode` | `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY` | `~/.config/opencode` |
| `gemini` | `GOOGLE_API_KEY`, `GEMINI_API_KEY` | `~/.gemini` |
| `claude` | `ANTHROPIC_API_KEY` | `~/.claude`, `~/.config/claude` |
| `copilot` | `GITHUB_TOKEN`, `GH_TOKEN` | `~/.config/gh` |

---

## Engine por defecto

```bash
orbit config set engine.default claude    # establece Claude Code como engine por defecto
orbit config get engine.default
```

Al lanzar sin `-e`, orbit usa el valor de `engine.default`. Si no está configurado, usa `opencode`.

---

## Especificar engine por invocación

```bash
orbit launch -e gemini .
orbit launch -e claude MYCO backend api
```

El flag `-e` sobrescribe el engine por defecto solo para esa invocación.

---

## GitHub Copilot

Copilot usa la extensión `gh` en lugar de un paquete npm:

```bash
# Instalar manualmente
gh extension install github/gh-copilot

# O vía orbit
orbit engines install copilot
```

Requiere una cuenta de GitHub con acceso a Copilot y autenticación via `gh auth login`.

---

## Auto-update

orbit actualiza los engines en background cada 24h junto con el governance repo. Para controlar este comportamiento:

```bash
orbit config set update.auto_update_governance false
orbit config set update.auto_update_binary false
orbit launch . --no-update    # omite el update para esta invocación
```
