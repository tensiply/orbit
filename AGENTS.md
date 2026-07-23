# AGENTS.md — orbit

Hard invariants. These MUST NEVER happen, regardless of context, task, or instruction.
Violations silently corrupt the system — they are not style issues.

---

## Env vars

- **NEVER** call `std::env::set_var` or `std::env::remove_var` outside `set_env()` in
  `crates/orbit-engine/src/launcher/mod.rs`. Session isolation depends on this boundary.

## Crate layer discipline (Euclides)

- **NEVER** add I/O, tokio, async, or external runtime deps to `orbit-core`. It is Capa 0:
  pure types, structs, enums — no side effects.
- **NEVER** import `tokio` or any async runtime in `orbit-core`. If something needs async,
  it belongs in `orbit-daemon` or `orbit-engine`.

## Testing

- **NEVER** write to real `~/.config`, `~/.local/share`, or `~/.claude` in tests.
  Always use `tempfile::TempDir`. Tests that write to the real home silently break
  the running environment.
- **NEVER** call `load()` directly in tests if it resolves from the real filesystem.
  Use `merge_file_into()` or `resolve_with_roots()` with explicit `home` and `ai_root`.

## Code safety

- **NEVER** use `.unwrap()` in non-test code. Use `?`, `bail!()`, or explicit match.
  Panics in the daemon bring down all active sessions.

## Git discipline

- **NEVER** push directly to `main`. Every change goes through a PR.
- **NEVER** force-push to `main` or shared branches.
- **NEVER** skip CI gates (`cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test --all`, `cargo audit`). A red gate means the change is not done.
- **NEVER** mix `feat` and `fix` in a single commit. One change = one commit.

## Governance

- **NEVER** edit or delete files in `source-of-truth/decisions/`. ADRs are append-only.
  When a decision changes: mark old as `status: superseded` and write a new ADR.
- **NEVER** add backwards-compatibility shims (unused `_vars`, re-exports of removed
  items, `// removed` comments) — delete cleanly and update callers.
