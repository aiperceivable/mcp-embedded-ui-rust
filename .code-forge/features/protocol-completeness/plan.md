# Implementation Plan: Protocol Completeness

**Source**: `../mcp-embedded-ui` (spec repo — PROTOCOL.md + feature specs F1–F6)
**Target**: `mcp-embedded-ui-rust` (Axum-based Rust port)
**Date**: 2026-03-16

---

## Analysis Summary

The Rust implementation is **~90% complete**. All core endpoints work, HTML
template rendering is correct, auth hooks function, and 39 tests pass.

### Gaps Identified

| # | Gap | Spec Reference | Severity |
|---|-----|---------------|----------|
| G1 | `WithRequest` handler variant is defined but returns an error at runtime | F3 §ToolCallHandler, PROTOCOL.md §ToolCallHandler | **P0** — advertised API doesn't work |
| G2 | `create_mount()` Tier 3 factory function is missing | F5 §Tier 3, PROTOCOL.md §Output | **P1** — convenience API gap |
| G3 | `create_mount` not exported from `lib.rs` | F5 §Public Type Exports | **P1** — blocked by G2 |

### Already Complete

- F1: HTML Frontend (template rendering, XSS escaping, template drift test)
- F2: Tool Discovery API (GET /tools, GET /tools/{name}, annotations omission)
- F3: Tool Execution API — Basic handler (precondition checks, error handling, trace_id)
- F4: Auth Hook (guards POST only, 401 without detail leak, server-side logging)
- F5: Tier 1 (`build_ui_routes`) and Tier 2 (`create_app`)
- F6: Try-It Console (frontend-only, in explorer.html)

---

## Tasks

### Task 1: Implement WithRequest handler support
**Feature**: F3 — Tool Execution API
**Files**: `src/server.rs`
**Complexity**: S

The `ToolCallHandler::WithRequest` variant exists in `types.rs` but `do_call()`
in `server.rs:150-156` returns a hardcoded error instead of forwarding the
request. The `call_tool` handler already receives headers and body but doesn't
reconstruct a Request for the WithRequest path.

#### TDD Steps

1. **RED** — Write `test_call_tool_with_request_handler` in `tests/test_server.rs`:
   - Create a `ToolCallHandler::WithRequest` that reads a custom header from the request
   - Call `POST /tools/echo/call` with that header
   - Assert 200 and the header value appears in the response content
   - *This test will FAIL because WithRequest returns an error*

2. **GREEN** — Modify `call_tool()` in `src/server.rs`:
   - Restructure to accept `Request` directly instead of separate `headers` + `body`
   - Extract headers and body from the request
   - In `do_call`, for `WithRequest` variant: reconstruct a `Request` from the
     extracted headers (body already consumed, but the handler needs headers/method/uri)
   - Alternative: change `call_tool` to pass the original `Request` to `do_call`
     for the WithRequest case

3. **REFACTOR** — Clean up:
   - Ensure `ToolCallWithRequestFn` signature aligns with actual usage
   - Verify no `unwrap()` in library paths

#### Acceptance Criteria

- [ ] `ToolCallHandler::WithRequest` executes the handler with the request
- [ ] Handler can access request headers
- [ ] All existing tests still pass
- [ ] No `unwrap()` / `expect()` in library code

---

### Task 2: Add `create_mount()` function
**Feature**: F5 — Framework Integration (Tier 3)
**Files**: `src/server.rs`, `src/lib.rs`
**Complexity**: S

Add a mount helper that wraps `build_ui_routes` with Axum's `.nest()`.
Default prefix is `"/explorer"`.

#### TDD Steps

1. **RED** — Write tests in `tests/test_server.rs`:
   - `test_create_mount_default_prefix`: call `GET /explorer/` and `GET /explorer/tools`,
     assert 200
   - `test_create_mount_custom_prefix`: use `"/ui"`, call `GET /ui/` and `GET /ui/tools`,
     assert 200

2. **GREEN** — Add `create_mount()` to `src/server.rs`:
   ```rust
   pub fn create_mount(
       prefix: Option<&str>,
       tools: Arc<dyn ToolsProvider>,
       handler: ToolCallHandler,
       config: UiConfig,
   ) -> Router {
       let prefix = prefix.unwrap_or("/explorer");
       Router::new().nest(prefix, build_ui_routes(tools, handler, config))
   }
   ```

3. **REFACTOR** — Export from `src/lib.rs`:
   ```rust
   pub use server::{build_ui_routes, create_app, create_mount};
   ```

#### Acceptance Criteria

- [ ] `create_mount()` with no prefix mounts at `/explorer`
- [ ] `create_mount()` with custom prefix mounts at that prefix
- [ ] All routes (HTML, tools, tool detail, call) work under the mounted prefix
- [ ] Function exported from crate root
- [ ] All existing tests still pass

---

### Task 3: Update example to use `create_mount`
**Feature**: F5 — Framework Integration
**Files**: `examples/axum_demo.rs`
**Complexity**: XS

Replace the manual `.nest("/explorer", build_ui_routes(...))` in the example
with `create_mount(None, ...)` or `create_mount(Some("/explorer"), ...)` to
demonstrate the Tier 3 API.

#### Steps

1. Update `examples/axum_demo.rs` to use `create_mount`
2. Verify `cargo build --examples` passes

#### Acceptance Criteria

- [ ] Example uses `create_mount` instead of manual `.nest()`
- [ ] Example builds cleanly

---

## Execution Order

```
Task 1 (WithRequest handler)  →  Task 2 (create_mount)  →  Task 3 (update example)
```

Tasks 1 and 2 are independent and could be parallelized, but Task 3 depends on
Task 2.

---

## Quality Gates

After all tasks:

| Command | Must Pass |
|---------|-----------|
| `cargo fmt --all -- --check` | Yes |
| `cargo clippy --all-targets --all-features -- -D warnings` | Yes |
| `cargo build --all-features` | Yes |
| `cargo test --all-features` | Yes |
| `cargo build --examples` | Yes |
