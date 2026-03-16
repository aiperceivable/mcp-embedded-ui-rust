# CLAUDE.md — mcp-embedded-ui-rust Development & Code Quality Specification

## Project Overview

`mcp-embedded-ui` is a **lightweight embedded Web UI for any MCP Server** — it provides HTTP routes that serve a self-contained HTML explorer page and JSON API endpoints for listing, inspecting, and executing MCP tools.

This is the **Rust implementation**. The canonical protocol is defined in `../mcp-embedded-ui/docs/PROTOCOL.md`.

---

## Reference Implementation

The Python implementation at `../mcp-embedded-ui-python/` is the reference. The Rust port must maintain protocol compatibility: same endpoints, same JSON shapes, same HTML template.

---

## Core Principles

- Prioritize **simplicity, readability, and maintainability** above all.
- Avoid premature abstraction, optimization, or over-engineering.
- Code should be understandable in ≤10 seconds; favor straightforward over clever.
- Always follow: **Understand → Plan → Implement minimally → Test/Validate → Commit**.

---

## Rust Code Quality

### Readability

- Use precise, full-word names; standard abbreviations only when idiomatic (`buf`, `cfg`, `ctx`).
- Functions ≤50 lines, single responsibility, verb-named (`parse_request`, `build_schema`).
- Avoid obscure tricks, overly chained iterators, unnecessary macros, or excessive generics.
- Break complex logic into small, well-named helper functions.

### Types (Mandatory)

- Provide explicit types on all public items; do not rely on inference for public API surfaces.
- Prefer `struct` over raw tuples for anything with more than 2 fields.
- Implement `serde::Serialize` / `serde::Deserialize` on all public data types.

### Design

- Favor **composition over inheritance**; use `trait` only for true behavioral interfaces.
- No circular module dependencies.
- Keep `pub` surface minimal.

### Errors & Resources

- Define domain errors with **`thiserror`**; no bare `Box<dyn Error>` in library code.
- Propagate errors with `?`; no `unwrap()` / `expect()` in library paths (tests excepted).

### Async

- Runtime: **Tokio** (`features = ["full"]`).
- Traits with async methods: use **`async-trait`**.

### Logging

- Use **`tracing`** — no `println!` / `eprintln!` in production code.

### Testing

- Run with: `cargo test --all-features`
- **Integration tests**: in `tests/` directory.
- **Examples**: in `examples/`; must build cleanly via `cargo build --examples`.
- Test names: `test_<unit>_<behavior>` (e.g., `test_call_tool_returns_200_on_success`).

---

## Mandatory Quality Gates

| Command | Purpose |
|---------|---------|
| `cargo fmt --all -- --check` | Formatting |
| `cargo clippy --all-targets --all-features -- -D warnings` | Lint |
| `cargo build --all-features` | Full build |
| `cargo test --all-features` | Tests |
| `cargo build --examples` | Example build |

---

## General Guidelines

- **English only** for all code, comments, doc comments, error messages, and commit messages.
- Fully understand surrounding code before making changes.
- No secrets hardcoded.
