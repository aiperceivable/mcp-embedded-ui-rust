# Changelog

All notable changes to this project will be documented in this file.

## [0.5.0] - 2026-08-20

### Changed

- **BREAKING (spec F6/FR-1): the Try-It editor prefill no longer fabricates values.**
  Synced `explorer.html` from the spec repo at 0.5.0. The prefill now emits exactly
  the keys listed in `inputSchema.required`, using each property's declared
  `default` when it has one and `null` otherwise. Optional properties are omitted
  entirely, generation does not recurse into nested objects, and a schema with no
  `required` prefills `{}`.

  The previous rule invented a type-based value for *every* property
  (`"string"` → `""`, `"number"` → `0`, …), which had two consequences. First,
  size: a 257-property schema produced a 259-line prefill inside a 120px editor.
  Second, and more seriously, it emitted a key for every property and drew every
  value from the declared type, so it satisfied `required` and the type
  constraints unconditionally — making the 0.4.0 Validate button incapable of
  failing on a fresh prefill for any schema. `null` supplies the key without
  asserting a value and is rejected wherever the schema does not admit it.

### Documentation

- **Recorded why `Authenticator` exists alongside `AuthHook`** on the trait
  itself: `AuthHook` receives no continuation and returns no value, so identity
  propagation through it is structurally impossible in Rust, whereas Python's
  context manager and TypeScript's `(req, next)` both carry the call. The trait
  is a parity mechanism so that Rust callers get the same API as the other two
  bindings — not extra capability. Also corrected `UiConfig.auth_hook`'s bare
  "Legacy" label, which read as deprecation: it is still the right choice for a
  pure pass/fail gate.

### Fixed

- **`project_url` is now scheme-checked before being placed in `href`.** Only
  `http://`, `https://`, `mailto:` and a leading `/` are accepted; anything else
  renders the project name as plain text. TAB/LF/CR are stripped and the value
  trimmed before the check, because browsers ignore those while resolving a
  scheme. Not an exploitable vulnerability — `project_url` is deployment
  configuration, not caller input — but HTML escaping alone never stopped
  `javascript:`.

- **`/validate` no longer mishandles a tool whose `inputSchema` cannot be
  compiled.** Such a schema is now reported as a single `keyword: "schema"`
  validation failure at HTTP 200, per the new F7 contract.
  Previously the `Err` arm returned an empty error list, reporting the input
  **valid** without having validated anything.
- An explicit `default: null` in a schema is now honoured. The previous guard
  (`props[key]['default'] != null`) discarded it and fell through to a fabricated
  type default.

### Tests

- **Covered the `Authenticator` path, which had no tests in this crate.** Six
  cases: identity actually reaches the handler (asserted on the observed
  identity, not just a 200), rejection returns 401, the authenticator takes
  precedence and the `AuthHook` provably does not run, the identity slot reads
  as empty rather than panicking when no authenticator is set, and neither the
  GET endpoints nor `/validate` are guarded. Maps to the new F4 TC-7..TC-11.
  Adds `async-trait` as a dev-dependency.

- Added a `/validate` case for a tool whose `inputSchema` cannot be compiled —
  asserts the input is reported invalid rather than silently valid.

- Added template guards for FR-1: the prefill must read `inputSchema.required`
  and must not fabricate type-based values. These run even when the spec repo is
  not checked out alongside, unlike the existing drift check.

## [0.4.0] - 2026-04-28

### Added

- **`POST /tools/{name}/validate` endpoint** — implements F7 from the spec. Validates request args against the tool's `inputSchema` without invoking the handler, returns `{"valid": true}` or `{"valid": false, "errors": [...]}`. Not gated by `allow_execute`, `auth_hook`, or `Authenticator` (per F7 spec). Adds `jsonschema = "0.46"` dependency (no default features).
- **`ValidateResult` and `ValidationFailure`** types re-exported from the crate root for callers that want to consume the response shape directly.
- **`explorer.html`** — synced from spec repo; gains the Validate button next to Execute.

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
