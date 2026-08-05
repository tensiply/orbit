# Activity Log

Historial persistente de sesiones de trabajo, organizado por scope, con inyección automática en el contexto de cada `orbit launch`.

---

## Qué es

El activity log es un registro append-only de sesiones de trabajo. Cada entrada indica qué se hizo, cuándo y en qué scope (tenant/project/repo). El launcher inyecta las últimas 5 entradas como contexto al iniciar un engine, de modo que el AI tiene visibilidad del trabajo reciente sin necesidad de preguntar.

**Almacenamiento:**
```
~/.orbit/data/workspaces/<slug>/activity/index.jsonl
```

Cada línea es un objeto JSON:
```json
{"ts": 1784851200, "scope": "AIDEV/AI-ECOSYSTEM/orbit", "summary": "Implementé el sistema de activity log", "session_id": "abc123"}
```

---

## Comandos

```bash
# Ver historial
orbit activity list                                   # últimas 10 entradas del workspace
orbit activity list --scope aidev/ai-ecosystem/orbit  # filtrar por scope
orbit activity list --limit 5                         # limitar cantidad
orbit activity list --md                              # output markdown para scripts

# Agregar una entrada manualmente
orbit activity append --summary "Implementé el feature X"
orbit activity append \
    --scope "AIDEV/AI-ECOSYSTEM/orbit" \
    --summary "Fix de Y bug" \
    --session-id "$SESSION_ID"

# Verificar si una sesión ya tiene entrada (usado por el hook)
orbit activity has --session-id "$SESSION_ID"    # exit 0 = existe, exit 1 = no existe
```

---

## Flujo automático

El activity log se llena solo sin intervención manual:

```
orbit launch ...
  └─ El launcher inyecta activity-context.md (últimas 5 entradas del scope)
       └─ El AI engine arranca con historial visible en contexto

/session-start
  └─ Paso 0: orbit activity list --limit 5
       └─ Muestra las últimas sesiones al iniciar

/session-close
  └─ Paso 6: orbit activity append --summary "..."
       └─ Registra un resumen de lo que se hizo

Stop hook (activity-log)
  └─ Si ya hay entrada para el session_id → no hace nada
  └─ Si no → escribe un stub mínimo "Session closed (no summary)"
```

Lo que el engine ve en su contexto al arrancar:

```markdown
## Actividad reciente

- **2026-08-04 15:50** `AIDEV/AI-ECOSYSTEM/orbit` — Fix en flujo de instalación y centralización de paths
- **2026-08-04 13:25** `AIDEV/AI-ECOSYSTEM/orbit` — MCP bugs: mcpServers para todos los engines, ScopeLevel::Workspace
```

---

## Activar el hook automático

Para que el activity log se llene automáticamente al cerrar cada sesión de Claude Code:

```bash
orbit hooks enable activity-log
```

El hook es asíncrono (no bloquea el cierre) y solo escribe si `/session-close` no registró ya una entrada para esa sesión.

---

## Scope key

El scope se construye como `TENANT/PROJECT/REPO` desde las variables de entorno que orbit inyecta al lanzar. Los campos vacíos se omiten:

| Nivel              | Scope key                  |
|--------------------|----------------------------|
| Tenant solo        | `AIDEV`                    |
| Tenant + project   | `AIDEV/AI-ECOSYSTEM`       |
| Completo           | `AIDEV/AI-ECOSYSTEM/orbit` |
