---
description: Close a session — capture reusable learnings and update context
agent: plan
---

Close the current work session for $ARGUMENTS. Answer these questions and act on the results:

1. **¿Qué aprendí que pueda reutilizarse?**
   Identify any new conventions, decisions, debugging patterns, or workflow insights discovered this session.

2. **¿Pertenece a la base común o al tenant/repo?**
   - Shared across all workspaces → `~/AI/source-of-truth/`
   - Specific to this tenant/project/repo → the appropriate scope layer

3. **¿Qué archivos deben actualizarse?**
   Update or create the relevant source-of-truth files. Follow the maintain-knowledge-index rule: add entries to `knowledge-index.md` for any new files created.

4. **¿Qué cambió en herramientas, flujo o convenciones?**
   If a tool, MCP, or workflow changed, update the corresponding reference file.

5. **¿Qué NO documentar?**
   Skip ephemeral details, in-progress state, and anything already in git history or CLAUDE.md.

6. **Registrar en el log de actividad** — Redactar un resumen de 1-3 líneas de lo realizado en esta sesión (qué se implementó, qué se corrigió, qué se decidió) y ejecutar:
   ```bash
   orbit activity append --summary "<resumen>"
   ```
   El scope y workspace se detectan automáticamente desde las variables de entorno de orbit (`AI_TENANT`, `AI_PROJECT`, `AI_REPOSITORY`, `AI_WORKSPACE_ROOT`). Si hay `$CLAUDE_SESSION_ID` o `$SESSION_ID` disponible, agregar `--session-id <id>`.

Leave the context better than at the start of the session.
