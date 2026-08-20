# mcp-embedded-ui (Rust)

The Rust implementation of [mcp-embedded-ui](https://github.com/aiperceivable/mcp-embedded-ui) — a browser-based tool explorer for any [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) server.

## What is this?

If you build an MCP server in Rust, your users interact with tools through raw JSON — no visual feedback, no schema browser, no quick way to test. This library adds a full browser UI to your server with **one import and one mount**.

```
┌───────────────────────────────────┐
│  Browser                          │
│  Tool list → Schema → Try it      │
└──────────────┬────────────────────┘
               │ HTTP / JSON
┌──────────────▼────────────────────┐
│  Your Rust MCP Server             │
│  + mcp-embedded-ui                │
│    (Axum)                         │
└───────────────────────────────────┘
```

## What does the UI provide?

- **Tool list** — browse all registered tools with descriptions and annotation badges
- **Schema inspector** — expand any tool to view its full JSON Schema (`inputSchema`)
- **Try-it console** — type JSON arguments, execute the tool, see results instantly
- **cURL export** — copy a ready-made cURL command for any execution
- **Auth support** — enter a Bearer token in the UI, sent with all requests

No build step. No CDN. No external dependencies. The entire UI is a single self-contained HTML page embedded in the crate.

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
mcp-embedded-ui = "0.4"
```

Requires [Axum](https://github.com/tokio-rs/axum) 0.8+ and [Tokio](https://tokio.rs/) 1.x.

## Quick Start

### Axum

```rust
use std::sync::Arc;
use mcp_embedded_ui::{create_mount, ToolsProvider, UiConfig};

// Mount at /explorer (default), enable tool execution
let tools: Arc<dyn ToolsProvider> = Arc::new(my_tools);
let config = UiConfig {
    allow_execute: true,
    ..UiConfig::default()
};
let app = create_mount(None, tools, my_handler, config);

// Or specify a custom prefix
let app = create_mount(Some("/mcp-ui"), tools, my_handler, config);

// Visit http://localhost:8000/explorer/
```

### Standalone router

```rust
use mcp_embedded_ui::{create_app, UiConfig};

// Returns an Axum Router — nest in any Axum application
let app = create_app(tools, my_handler, UiConfig::default());
```

### Full working example

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use mcp_embedded_ui::{
    create_mount, Content, HandlerResult, Tool, ToolCallError,
    ToolCallHandler, ToolsProvider, UiConfig,
};

// 1. Define your tools (implement the Tool trait)
struct GreetTool;
impl Tool for GreetTool {
    fn name(&self) -> &str { "greet" }
    fn description(&self) -> &str { "Say hello" }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        })
    }
}

// 2. Define a handler: (name, args) -> (content, is_error, trace_id)
let handler = ToolCallHandler::Basic(Arc::new(|name, args| -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
    Box::pin(async move {
        let msg = args.get("name").and_then(|v| v.as_str()).unwrap_or("world");
        Ok((
            vec![Content { content_type: "text".into(), text: Some(format!("Hello, {}!", msg)), mime_type: None, data: None }],
            false,
            None,
        ))
    })
}));

// 3. Mount the UI
let tools: Arc<dyn ToolsProvider> = Arc::new(vec![Arc::new(GreetTool) as Arc<dyn Tool>]);
let config = UiConfig { allow_execute: true, ..UiConfig::default() };
let app = create_mount(None, tools, handler, config);
```

### With auth hook

```rust
use mcp_embedded_ui::{AuthHook, AuthError};

let config = UiConfig {
    allow_execute: true,
    auth_hook: AuthHook(Some(Arc::new(|parts| {
        Box::pin(async move {
            let auth = parts.headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if auth.starts_with("Bearer ") {
                Ok(())
            } else {
                Err(AuthError)
            }
        })
    }))),
    ..UiConfig::default()
};
```

Auth only guards `POST /tools/{name}/call`. Discovery endpoints are always public. The UI has a built-in token input field — enter your Bearer token there and it's sent with every execution request.

The included demo (`examples/axum_demo.rs`) uses a hardcoded `Bearer demo-secret-token` — the token is printed at startup so you know what to paste into the UI.

### Dynamic tools

```rust
use mcp_embedded_ui::DynamicToolsProvider;

// Async callable — re-evaluated on every request
let provider = DynamicToolsProvider::new(|| {
    Box::pin(async { registry.list_tools().await })
});
let tools: Arc<dyn ToolsProvider> = Arc::new(provider);
```

## API

### Three-tier API

| Function | Returns | Use case |
|----------|---------|----------|
| `create_mount(prefix, tools, handler, config)` | `Router` | Axum — nest under a URL prefix |
| `create_app(tools, handler, config)` | `Router` | Standalone Axum router |
| `build_ui_routes(tools, handler, config)` | `Router` | Power users — fine-grained route control |

### Parameters (`UiConfig`)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `allow_execute` | `bool` | `false` | Enable/disable tool execution (enforced server-side) |
| `title` | `String` | `"MCP Tool Explorer"` | Page title (HTML-escaped automatically) |
| `project_name` | `Option<String>` | `None` | Project name shown in footer |
| `project_url` | `Option<String>` | `None` | Project URL linked in footer (requires `project_name`) |
| `auth_hook` | `AuthHook` | `None` | Legacy async auth guard (validation only) |
| `authenticator` | `Option<Arc<dyn Authenticator>>` | `None` | Full auth with identity propagation (recommended) |

### Authenticator (recommended)

Implement the `Authenticator` trait to authenticate requests and propagate an `Identity` to tool call handlers via the `AUTH_IDENTITY` task-local:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use mcp_embedded_ui::{Authenticator, Identity, AUTH_IDENTITY, UiConfig};

struct MyAuth;

#[async_trait]
impl Authenticator for MyAuth {
    async fn authenticate(&self, headers: &HashMap<String, String>) -> Option<Identity> {
        let token = headers.get("authorization")?.strip_prefix("Bearer ")?;
        // Validate token, return Identity on success
        Some(Identity {
            id: "user-123".into(),
            identity_type: "human".into(),
            roles: vec!["user".into()],
            attrs: Default::default(),
        })
    }
}

let config = UiConfig {
    allow_execute: true,
    authenticator: Some(Arc::new(MyAuth)),
    ..UiConfig::default()
};
```

Inside tool call handlers, read the authenticated identity:

```rust
let identity = AUTH_IDENTITY.try_with(|id| id.clone()).ok().flatten();
```

When `authenticator` is set, it takes precedence over `auth_hook`. Returning `None` from `authenticate()` responds with 401.

### Auth Hook (legacy)

The `auth_hook` receives the request `Parts` (headers, URI, method) and returns a future resolving to `Result<(), AuthError>`. Return `Err(AuthError)` to reject with 401. Unlike `Authenticator`, it does not propagate identity to handlers. The error response is always `{"error": "Unauthorized"}` — internal details are never leaked.

Auth only guards `POST /tools/{name}/call`. Discovery endpoints (`GET /tools`, `GET /tools/{name}`) are always public.

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Self-contained HTML explorer page |
| GET | `/tools` | Summary list of all tools |
| GET | `/tools/{name}` | Full tool detail with `inputSchema` |
| POST | `/tools/{name}/call` | Execute a tool, returns MCP `CallToolResult` |

## Development

```bash
# Run the demo (auth enabled with a demo token)
cargo run --example axum_demo
# Visit http://localhost:8000/explorer/
# Paste "Bearer demo-secret-token" in the UI's token field to execute tools

# Format check
cargo fmt --all -- --check

# Run tests
cargo test --all-features

# Lint
cargo clippy --all-targets --all-features -- -D warnings
```

## Cross-Language Specification

This crate implements the [mcp-embedded-ui](https://github.com/aiperceivable/mcp-embedded-ui) specification. The spec repo contains:

- [PROTOCOL.md](https://github.com/aiperceivable/mcp-embedded-ui/blob/main/docs/PROTOCOL.md) — endpoint spec, data shapes, security checklist
- [explorer.html](https://github.com/aiperceivable/mcp-embedded-ui/blob/main/docs/explorer.html) — shared HTML template (identical across all language implementations)
- [Feature specs](https://github.com/aiperceivable/mcp-embedded-ui/blob/main/docs/features/MANIFEST.md) — detailed requirements and test criteria

## License

Apache-2.0
