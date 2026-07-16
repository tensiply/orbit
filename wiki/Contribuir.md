# Contribuir

## Setup de desarrollo

```bash
git clone https://github.com/befraeloircorona/orbit.git
cd orbit
cargo build                         # build debug
cargo build --release               # build release
cargo test --all                    # todos los tests
```

Requiere Rust 1.75+.

### Usar el binario de desarrollo

```bash
cargo build
orbit mode dev ./target/debug/orbit     # symlink al build local
orbit mode status                       # verifica que está en modo dev
```

---

## CI gates

Todos deben pasar antes de merge:

```bash
cargo fmt --all --check             # formato
cargo clippy --all-targets --all-features -- -D warnings   # lints
cargo test --all                    # tests
cargo audit                         # vulnerabilidades de dependencias
```

Correr localmente antes de un PR:

```bash
make check      # o ejecutar los comandos anteriores en secuencia
```

---

## Convenciones de código

### Error handling

- `anyhow::Result` dentro de crates y en entry points de binarios
- `thiserror` solo para errores tipados en `orbit-core::error` que cruzan boundaries de API
- `bail!()` para salidas tempranas con mensaje; `?` para propagar
- Nunca `.unwrap()` en código no-test — usar `?` o match explícito

### Paths

- Siempre `PathBuf`, nunca `String` para rutas
- Resolver paths relativos a través de `normalize_path()` en `orbit-engine::config`
- Matching de directorios case-insensitive: usar `find_dir_icase()` en `resolver.rs`

### Testing

- Unit tests: módulo `#[cfg(test)]` al final de cada archivo
- Tests de filesystem: siempre `tempfile::TempDir` — nunca escribir a `~/.config` real
- Resolución de scope: usar `resolve_with_roots()` con `home` y `ai_root` explícitos

### Naming

- Structs y enums: `PascalCase`
- Funciones y variables: `snake_case`
- Módulos de crates: `snake_case`, una palabra preferida (`config`, `context`, `launcher`)
- Campos de scope en código: `tenant`, `project`, `repository` (no `repo` como nombre de campo)

### Comentarios

- Sin comentarios describiendo QUÉ hace el código — los nombres lo hacen
- Separadores de sección: `// ── section name ──────────────────`
- Un comentario de doc corto para APIs públicas; sin docstrings multi-párrafo
- Los invariantes no obvios SÍ merecen un comentario corto

---

## Convenciones de commits

orbit usa [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <descripción>
```

### Tipos

| Tipo | Cuándo |
|---|---|
| `feat` | Nueva funcionalidad |
| `fix` | Corrección de bug |
| `refactor` | Refactor sin cambio de comportamiento |
| `perf` | Mejora de rendimiento |
| `test` | Tests |
| `docs` | Documentación |
| `chore` | Dependencias, CI, tareas de mantenimiento |

### Scopes por área

| Área | Scope |
|---|---|
| `orbit-engine/src/launcher/` | `launcher` |
| `orbit-engine/src/` (non-launcher) | `engine` |
| Carga de config / parsing JSONC | `config` |
| Resolución de scope | `resolver` |
| `orbit-tui/` | `tui` |
| `orbit-daemon/` | `daemon` |
| `orbit-cli/` | `cli` |
| `orbit-core/` | `core` |
| `orbit-cli/src/commands/env.rs` | `env` |
| `orbit-cli/src/commands/secret.rs` | `secret` |
| `orbit-cli/src/commands/plan.rs` | `planner` |
| `orbit-cli/src/commands/serve.rs` | `serve` |

### Ejemplos

```bash
git commit -m "feat(engine): add session timeout support"
git commit -m "fix(config): resolve XDG_CONFIG_HOME override on macOS"
git commit -m "chore(deps): bump serde to 1.0.203"
```

---

## Pull requests

1. Crear una rama desde `main`
2. Un PR = un cambio coherente (no mezclar feat + fix)
3. Asegurarse de que todos los CI gates pasen
4. Título del PR en formato Conventional Commits

---

## Release

Los releases se crean desde `main` usando el [Git Release workflow](https://github.com/befraeloircorona/orbit/wiki/Releases). El workflow de CI compila binarios para Linux (x86_64, aarch64) y macOS (x86_64, aarch64).

```bash
# Calcular la nueva versión (SemVer desde commits)
# BREAKING CHANGE → MAJOR, feat → MINOR, fix → PATCH
git tag -a v<VERSION> -m "chore: release v<VERSION>"
git push origin v<VERSION>
```
