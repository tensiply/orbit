# Sistema de planes

El sistema de planes permite crear y ejecutar tareas de IA de forma autónoma, con control de presupuesto, historial de auditoría y templates reutilizables.

---

## Conceptos

| Concepto | Descripción |
|---|---|
| **Plan** | Descripción de una tarea a ejecutar por el engine de IA |
| **Ejecución** | Una corrida de un plan, con contexto, presupuesto y resultado |
| **Template** | Plan predefinido con estructura reutilizable |
| **Memoria** | Historial de ejecuciones pasadas, consultable |
| **Executor** | Plugin que materializa cómo se ejecuta el plan |
| **Planner** | Componente de IA que descompone intenciones en planes estructurados |

---

## `orbit plan`

```bash
orbit plan create "implementar tests para el módulo auth"
orbit plan list                         # lista planes del scope actual
orbit plan show <id>                    # muestra detalles de un plan
orbit plan run <id>                     # ejecuta un plan
orbit plan status <id>                  # estado de ejecución
```

### Templates disponibles

Los templates están incluidos en el binario y cubren los flujos más comunes de desarrollo:

| Template | Descripción |
|---|---|
| `pr` | Prepara y crea un pull request |
| `code` | Implementación de código a partir de una descripción |
| `verify` | Verifica que un cambio funciona correctamente |
| `test` | Escribe o actualiza tests |
| `review` | Revisa código en busca de bugs y mejoras |

```bash
orbit plan create --template pr "agregar feature X"
orbit plan create --template test "módulo de autenticación"
```

---

## `orbit memory`

El sistema de memoria guarda el historial de todas las ejecuciones de planes, con costo, resultado y contexto.

```bash
orbit memory list                       # lista ejecuciones recientes
orbit memory search "auth"              # busca ejecuciones por texto
orbit memory show <id>                  # detalle completo de una ejecución
```

---

## Control de presupuesto

Cada plan puede tener un presupuesto máximo de tokens/costo. orbit detiene la ejecución si se supera el límite.

```bash
orbit plan run <id> --budget 50000      # límite de 50k tokens
orbit plan run <id> --budget 0.50       # límite de $0.50
```

El sistema registra el costo real de cada ejecución en la memoria de auditoría.

---

## Scheduling y webhooks

Los planes pueden programarse para ejecutarse de forma recurrente o dispararse vía webhook:

```bash
orbit plan create --schedule "0 9 * * 1-5" "resumen diario de PRs"    # cron
orbit plan create --webhook <url> "revisar commits nuevos"              # webhook
```

---

## Notificaciones

orbit puede enviar notificaciones de escritorio al completar una ejecución:

```bash
orbit plan run <id> --notify
orbit notify "mensaje personalizado"
```

---

## Vista multi-scope

```bash
orbit plan list --all-scopes    # planes de todos los scopes accesibles
```

Los planes se almacenan por scope. La vista multi-scope muestra planes de todos los scopes del workspace.

---

## Executor plugins

El sistema de planes soporta plugins de executor que definen cómo se materializa la ejecución. El executor por defecto usa el engine activo (claude, gemini, etc.). Plugins custom pueden conectar otros backends.

---

## Evaluación en CI

El crate `orbit-eval` permite validar la estructura de planes sin ejecutar el engine — útil para verificar templates y planes en pipelines de CI:

```bash
orbit plan eval <archivo.plan>      # valida estructura sin ejecutar
```
