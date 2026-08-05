# Roadmap

Features planeadas para orbit y el ecosistema AI.

---

## Próximas features del core (near-term)

Features diseñadas e implementadas parcialmente:

### `orbit snapshot` — Sincronización de contexto
Sincroniza los archivos de contexto del engine (`CLAUDE.md`, `AGENTS.md`) con el source-of-truth del scope. Útil para mantener coherentes los archivos que el AI lee al arrancar con el estado documentado en governance.

```bash
orbit snapshot              # sincroniza el scope actual
orbit snapshot --dry-run    # muestra qué cambiaría sin aplicar
```

### `orbit document` — Generación de documentos
Genera documentos (PDF, HTML, DOCX, XLSX, CSV) desde templates YAML. Las reglas de generación viven en el source-of-truth del scope.

```bash
orbit document create <template>
orbit document list
orbit document update <id>
```

### `orbit notify` — Notificaciones desktop
Notificaciones de sistema al terminar la ejecución de un plan. Integrado con el planner — dispara cuando un nodo llega a estado terminal (success, failure, cancelled).

### `orbit workspace` — Registry visual
Gestión de workspaces desde el TUI y la CLI. Permite agregar, eliminar y renombrar workspaces sin editar manualmente `~/.orbit/workspaces.toml`.

```bash
orbit workspace list
orbit workspace add ~/trabajo --name trabajo
orbit workspace remove trabajo
```

### `orbit serve` / `orbit discover` — LAN collaboration
Compartir contexto y trabajo con otros en la misma red local (Tab 9 del TUI). Un nodo actúa como servidor y los demás lo descubren via mDNS.

```bash
orbit serve              # exponer el workspace en la red local
orbit discover           # listar workspaces disponibles en la red
```

---

## Ecosistema (productos futuros)

orbit es el punto de entrada unificado de un ecosistema de productos IA. Los productos del ecosistema comparten contexto, identidad y configuración a través de orbit.

```
                    ┌──────────────────┐
                    │   orbit (CLI)    │  ← gateway unificado
                    └────────┬─────────┘
                             │
                    ┌────────┴────────┐
                    │    AI Hub       │  ← backend de integraciones
                    └────────┬────────┘
          ┌──────────────────┼──────────────────┐
    ┌─────┴──────┐  ┌────────┴───┐  ┌───────────┴──┐
    │Teams Agent │  │AI Meetings │  │  Mobile Hub  │
    └────────────┘  └────────────┘  └──────────────┘
```

### AI Hub
Backend central de integraciones: conexiones internas y externas, compartir contexto entre dispositivos y usuarios, gestión de archivos de trabajo. Los productos de superficie (Teams, Meetings, Mobile) se conectan a través del hub.

### Teams Agent
Agente conversacional en Microsoft Teams. Acceso a documentación, servicios internos, APIs externas y dashboards directamente desde un chat de Teams. Comparte el contexto de orbit sin necesidad de abrir el CLI.

### AI Meetings
Asistente de reuniones: notas automáticas durante la llamada, extracción de action items, resúmenes post-call enviados al equipo. Integrado con Teams y calendario.

### Mobile Hub
Acceso al ecosistema orbit desde el celular. Consultas rápidas, notificaciones de planes y acceso al historial de actividad desde cualquier lugar.

---

## Backlog de ideas

Ideas en evaluación, sin fecha ni compromiso:

| Idea | Descripción |
|---|---|
| `ai-knowledge` | Base de conocimiento personal/equipo con búsqueda semántica sobre repos, docs, Confluence, Notion |
| `ai-email` | Gestión de email: triaje inteligente, borradores, seguimiento de hilos |
| `ai-workflow` | Automatizaciones disparadas por eventos: cron, webhooks, cambios en repos |
| `ai-analytics` | Dashboard personal: métricas de trabajo, tiempo, productividad, código |
| `ai-personas` | Personalidades del asistente por contexto (trabajo, personal, técnico, reunión) |
| `ai-ops` | Monitoreo y alertas inteligentes sobre servicios e infra propios |
| `ai-finance` | Seguimiento de gastos e inversiones con insights de IA |

---

## Mejoras pendientes al core

| Feature | Descripción |
|---|---|
| Plugin marketplace | Registro público de plugins para compartir con otros usuarios |
| Historial de planes con replay | Ver el historial completo de un plan y re-ejecutar pasos |
| Métricas por scope | Tiempo por scope, costo de tokens, frecuencia de uso |
| Más engines | GitHub Copilot, modelos Llama locales vía Ollama |
| Sincronización multi-dispositivo | Compartir contexto y workspaces entre máquinas vía AI Hub |
