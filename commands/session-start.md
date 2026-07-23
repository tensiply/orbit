---
description: Inicializar una sesión — verificar scope, contexto y objetivo
agent: plan
---

Inicializa la sesión de trabajo actual para $ARGUMENTS.

Pasos (ejecutar en paralelo donde sea posible):

1. Detectar el engine activo (Claude Code, OpenCode, Gemini).
2. Identificar el scope desde el directorio de trabajo: workspace → tenant → project → repositorio.
3. Confirmar que el contexto compartido cargó correctamente (governance, workflows, capas de source-of-truth).
4. Listar overrides locales activos para este scope (tenant/project/repo source-of-truth).
5. Verificar identidad git: `git config user.email` — advertir si no coincide con la identidad esperada del workspace según el workflow `git-identity.md`.
6. **Estado del repositorio** — ejecutar en paralelo:
   a. `git status --short` + rama actual (`git branch --show-current`) — archivos modificados, staged, sin trackear.
   b. `git log --oneline -5` — últimos commits para orientarse en el trabajo previo.
7. **PRs abiertos** — si GitHub MCP está disponible, listar PRs abiertos del repositorio actual. Si no está disponible, indicarlo brevemente.
8. **Estado de CI** — si GitHub MCP está disponible, obtener el estado de checks de la rama actual (`gh pr checks` o `gh run list --branch <branch> --limit 3`). Si no está disponible, indicarlo brevemente.
9. **Trabajo pendiente** — leer el `source-of-truth/README.md` y `knowledge-index.md` del scope activo. Resumir en 2-3 puntos: qué está en progreso, qué está pendiente, qué decisiones están abiertas.
10. Pedir al usuario que confirme o declare el objetivo de la sesión en una oración.

Salida: resumen breve con scope, engine, identidad, estado del repo, PRs activos, CI, trabajo pendiente, y objetivo de sesión.
