# Scope Catalog

El scope catalog es el índice de todos los repositorios registrados en orbit, organizados por la jerarquía workspace → tenant → project → repository.

---

## Qué es el scope

Orbit organiza el trabajo en una jerarquía de cuatro niveles:

```
WORKSPACE (~/AI, ~/trabajo, ...)
  └─ TENANT (AIDEV, DEVTEAM, ...)
       └─ PROJECT (AI-ECOSYSTEM, core, ...)
            └─ REPOSITORY (orbit, backend, ...)
```

Cada nivel tiene su propio `orbit.json` con instrucciones, variables de entorno y MCPs. El scope catalog es el índice completo de todos los repos conocidos por orbit, usado para auto-detectar el scope desde el directorio de trabajo.

---

## Comandos

### `orbit scope scan`

Escanea todos los workspaces registrados y reconstruye el catálogo en disco:

```bash
orbit scope scan
```

Salida:
```
  Scanning 2 workspace(s)...
    AI → /home/user/AI
    trabajo → /home/user/trabajo

  ✓  47 repositories indexed
  Catalog: ~/.orbit/cache/scope-catalog.json
```

Si detecta workspaces no registrados en `~/`, sugiere el comando para agregarlos.

---

### `orbit scope list`

Lista todos los repositorios del catálogo:

```bash
orbit scope list                      # todos
orbit scope list --workspace AI       # filtrar por workspace
orbit scope list --json               # output JSON para scripts
```

Salida:
```
  workspace  tenant  project        repository
  ──────────────────────────────────────────────────────
  AI         AIDEV   AI-ECOSYSTEM   orbit          CLI principal del ecosistema
             AIDEV   AI-ECOSYSTEM   ai-launcher    Launcher bash legacy
  ──────────────────────────────────────────────────────

  2 repositories
```

---

### `orbit scope check`

Valida la integridad de los archivos de governance en todos los scopes:

```bash
orbit scope check                     # todos los workspaces
orbit scope check --workspace AI      # solo un workspace
```

Reporta si faltan `orbit.json`, `source-of-truth/README.md`, `source-of-truth/conventions.md` u otros archivos esperados:

```
  3 governance issue(s):

  ◆  AI/AIDEV/core/backend      Missing orbit.json
  ◆  trabajo/DEVTEAM/reports      Missing source-of-truth/README.md
  ◆  AI/AIDEV/tools/scraper     Missing source-of-truth/conventions.md

  Run `orbit scope scan` after fixing to update the catalog.
```

---

## Auto-detección de scope

Cuando orbit necesita saber el scope actual (por ejemplo, `orbit activity list` sin argumentos), consulta el catálogo buscando el repo cuyo `local_path` coincide con el directorio de trabajo actual o alguno de sus padres.

`orbit launch` sin argumentos explícitos también usa este mecanismo para inferir el scope desde el directorio actual.

---

## Mantenimiento del catálogo

El catálogo se guarda en `~/.orbit/cache/scope-catalog.json` y no se actualiza automáticamente. Ejecuta `orbit scope scan` después de:

- Clonar un repo nuevo
- Agregar un workspace con `orbit workspace add`
- Mover directorios de repos

El catálogo es una caché derivable — puede regenerarse en cualquier momento con `scan` sin perder información.
