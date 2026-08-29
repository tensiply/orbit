# Contributing to Orbit

Thank you for your interest in contributing to Orbit. This document covers
everything you need to get started.

## Before You Contribute

### Contributor License Agreement (CLA)

All contributions require a signed CLA. Read [CLA.md](CLA.md) and sign it
by leaving the required comment on your pull request. Unsigned PRs will not
be merged.

### Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you agree to abide by its terms.

---

## Getting Started

### Prerequisites

- Rust (latest stable — see [rustup.rs](https://rustup.rs))
- `cargo`, `clippy`, `rustfmt` (included with rustup)

### Setup

```bash
git clone https://github.com/tensiply/orbit.git
cd orbit
cargo build
```

### Running tests

```bash
cargo test --all
```

### Formatting and linting

```bash
cargo fmt --all
cargo clippy --all -- -D warnings
```

All CI gates must pass before a PR can be merged:

```bash
cargo fmt --all --check
cargo clippy --all -- -D warnings
cargo test --all
cargo audit
```

---

## How to Contribute

### Reporting bugs

Search [existing issues](../../issues) first. If none match, open a new one
using the **Bug Report** template.

### Suggesting features

Open a [Feature Request](../../issues/new?template=feature_request.yml) with
context, motivation, and a rough description of the desired behavior.

### Submitting a pull request

1. Fork the repository and create a branch from `main`.
2. Make your changes. Keep PRs focused — one change per PR.
3. Ensure all CI gates pass locally before opening the PR.
4. Open the PR and sign the CLA in a comment (see [CLA.md](CLA.md)).
5. Describe your changes clearly in the PR description.

### Branch naming

Use descriptive names: `fix/session-resume-crash`, `feat/engine-timeout`,
`docs/contributing-guide`.

---

## Commit Style

This project uses [Conventional Commits](https://www.conventionalcommits.org):

```
feat(scope): add support for X
fix(scope): resolve Y when Z
docs(scope): update contributing guide
```

---

## Questions

Open a [Discussion](../../discussions) or reach out at
[orbit@tensiply.com](mailto:orbit@tensiply.com).
