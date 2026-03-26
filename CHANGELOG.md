# Changelog

All notable changes to this project will be documented in this file.

## [0.3.2] - 2026-03-26

### Changed

- Update `explorer.html` — sync cross-language implementation links from relative paths to absolute GitHub URLs.

## [0.3.1] - 2026-03-17

### Added

- **`Authenticator` trait** — full-featured authentication that returns an `Identity` on success, replacing the validation-only `AuthHook` pattern. When set on `UiConfig`, it takes precedence over the legacy `auth_hook`.
- **`Identity` struct** — represents an authenticated user or service with `id`, `identity_type`, `roles`, and `attrs` (arbitrary `HashMap<String, serde_json::Value>` attributes).
- **`AUTH_IDENTITY` task-local** — propagates the authenticated `Identity` to tool call handlers during execution. Handlers read it via `AUTH_IDENTITY.try_with(|id| id.clone())`.
- **`ToolCallFn` / `ToolCallWithRequestFn` re-exports** — previously defined but not exported from `lib.rs`.

### Changed

- **`UiConfig` gains `authenticator` field** — `Option<Arc<dyn Authenticator>>`, defaults to `None`. Fully backward compatible.
- **Auth precedence** — when both `authenticator` and `auth_hook` are set, `authenticator` wins. Legacy `auth_hook` is only evaluated as a fallback.

## [0.3.0] - 2026-03-11

### Added

- **Dark mode** — theme toggle button with light/dark switching, `localStorage` persistence, and system preference auto-detection (from updated shared HTML template).
- **`create_mount()` function** — convenience wrapper that nests the UI router under a URL prefix (default: `/explorer`).
- **`ToolCallHandler::WithRequest` variant** — 3-param handler that receives the full `Request` object for access to headers, URI, method, etc.
- **`DynamicToolsProvider`** — async function-backed tools provider, re-evaluated on every request.
- **`build_mcp_ui_routes()` deprecated alias** — backward-compatible entry point that delegates to `build_ui_routes()`.

### Changed

- **`allow_execute` default changed to `false`** — secure by default; callers must explicitly set `allow_execute: true` in `UiConfig` to enable tool execution.
- **`AuthHookFn` now receives `Parts`** — auth hooks receive full request `Parts` (headers, URI, method) instead of just `HeaderMap`, matching the protocol specification.
- **Precondition order** — `POST /tools/{name}/call` now follows the spec order: `allow_execute` check → tool lookup → JSON parse → auth hook → handler.

## [0.2.0] - 2026-03-10

### Added

- **`ToolCallHandler` enum** — supports both 2-param `Basic(name, args)` and 3-param `WithRequest(name, args, request)` handler variants.
- **`allow_execute`** parameter — defaults to `true`; set to `false` to disable tool execution server-side.
- **`project_name` / `project_url`** in `UiConfig` — optional footer link for downstream projects.
- **Tool search/filter, multi-content-type rendering, execution time display, cURL escaping fix** — all from updated shared HTML template.

### Changed

- `html.rs` renders HTML from `include_str!("explorer.html")` with template variable replacement.

## [0.1.0] - 2025-12-01

### Added

- Initial implementation with Axum routes, app factory, and mount helper.
- Tool discovery (`GET /tools`, `GET /tools/{name}`), execution (`POST /tools/{name}/call`), and auth hook support.
- Self-contained HTML explorer page with embedded CSS and JavaScript.
- Comprehensive integration test suite (34 tests).
