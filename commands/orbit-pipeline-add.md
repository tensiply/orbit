---
description: "Agrega un pipeline a orbit.json — guía interactiva para GitHub Actions o Jenkins en cualquier scope"
agent: implementation
---

Configura un pipeline CI/CD en el `orbit.json` del scope indicado.

## Paso 1 — Leer el request

Interpreta `$ARGUMENTS` si fue provisto. Extrae de él:
- Nombre del pipeline (ej. "CI", "Deploy", "Lint")
- Provider si se menciona (github actions / jenkins)
- Repo, branch, URL, job si se mencionan

Si `$ARGUMENTS` está vacío o incompleto, salta directamente al Paso 2.

## Paso 2 — Recopilar datos del pipeline

Pregunta al usuario lo que no puedas inferir de `$ARGUMENTS`. Hazlo en una sola ronda de preguntas (no una por una):

**Para todos los providers:**
- Nombre del pipeline (label visible en el badge del header de orbit desktop)
- Provider: `github_actions` o `jenkins`

**Si `github_actions`:**
- `repo`: `owner/repo` (ej. `tensiply/orbit`)
- `branch`: rama a monitorear (default: `main`)
- `workflow`: nombre del workflow a filtrar (opcional, deja vacío para todos)
- `token_secret`: referencia al secreto del token (ej. `keychain://GITHUB_TOKEN`). Si el usuario no lo sabe, sugiere `keychain://GITHUB_TOKEN` y explica que debe ejecutar `orbit secret set GITHUB_TOKEN <pat>`.

**Si `jenkins`:**
- `url`: URL base de Jenkins (ej. `https://jenkins.example.com`)
- `job`: path del job separado por `/` (ej. `folder/job-name` o `jafra/plus/jf-backend-auth`)
- `token_secret`: referencia al token de Jenkins (ej. `keychain://JENKINS_TOKEN`).

## Paso 3 — Determinar el scope target

Pregunta al usuario: "¿A qué scope quieres agregar este pipeline?"

Opciones:
- `workspace` → `~/AI/orbit.json`
- `tenant:<T>` → `~/AI/tenants/<T>/orbit.json`
- `project:<T>/<P>` → `~/AI/tenants/<T>/projects/<P>/orbit.json`
- `repo:<T>/<P>/<R>` → `~/AI/tenants/<T>/projects/<P>/repositories/<R>/orbit.json`

Si el usuario no especifica, detecta el scope desde las variables de entorno:
- `AI_TENANT`, `AI_PROJECT`, `AI_REPOSITORY` → usa el más profundo disponible.
- Si ninguna está disponible → usa `workspace`.

Resuelve la ruta completa del `orbit.json` destino.

## Paso 4 — Leer orbit.json existente

Lee el `orbit.json` en la ruta resuelta. Si no existe, trátalo como `{}`.

Verifica si ya existe un pipeline con el mismo nombre en el array `pipelines`. Si existe:
- Muestra el entry existente.
- Pregunta al usuario si quiere sobreescribir o cancelar.

## Paso 5 — Construir el entry JSON

Construye el objeto con solo los campos relevantes al provider (omite campos del otro provider):

Para `github_actions`:
```json
{
  "name": "<nombre>",
  "provider": "github_actions",
  "repo": "<owner/repo>",
  "branch": "<branch>",
  "token_secret": "<keychain://SECRET>"
}
```

Para `jenkins`:
```json
{
  "name": "<nombre>",
  "provider": "jenkins",
  "url": "<url>",
  "job": "<job/path>",
  "token_secret": "<keychain://SECRET>"
}
```

Omite campos opcionales que el usuario dejó vacíos (no incluyas `"workflow": null`).

## Paso 6 — Actualizar orbit.json

Agrega el entry al array `pipelines` del `orbit.json`. Si el array no existe, créalo.
Escribe el archivo con formato JSON indentado (2 espacios).

Preserva todos los campos existentes del archivo (`env`, `architecture`, etc.).

## Paso 7 — Verificar

Ejecuta:
```bash
orbit pipelines list
```

Si el pipeline aparece en la lista: éxito.
Si no aparece: muestra el contenido del `orbit.json` actualizado para depurar.

## Paso 8 — Instrucciones post-configuración

Si el `token_secret` usa `keychain://` y el usuario no mencionó que ya tiene el secreto guardado, muestra:

```
Para activar el fetch del token, ejecuta:
  orbit secret set <SECRET_KEY> <token>

Luego verifica el estado en tiempo real:
  orbit pipelines status
```

Muestra un resumen final:
- Archivo modificado (ruta completa)
- Pipeline agregado (nombre + provider)
- Scope donde aplica
- Comando para ver el estado: `orbit pipelines status`
