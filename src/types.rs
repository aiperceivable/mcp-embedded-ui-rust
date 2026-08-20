//! Public types for mcp-embedded-ui.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Request;
use serde::{Deserialize, Serialize};

/// A single content item in a tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResult {
    pub content: Vec<Content>,
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<CallResultMeta>,
}

/// Metadata attached to a call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResultMeta {
    #[serde(rename = "_trace_id")]
    pub trace_id: String,
}

/// Summary of a tool (returned by `GET /tools`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
}

/// Full detail of a tool (returned by `GET /tools/{name}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDetail {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
}

/// Describes a tool that can be served by the embedded UI.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn annotations(&self) -> Option<serde_json::Value> {
        None
    }
}

/// Boxed future returning a list of tools.
pub type ToolsFuture<'a> = Pin<Box<dyn Future<Output = Vec<Arc<dyn Tool>>> + Send + 'a>>;

/// Provides the list of tools. Can be static or dynamic.
pub trait ToolsProvider: Send + Sync + 'static {
    fn tools(&self) -> ToolsFuture<'_>;
}

/// Static list of tools.
impl ToolsProvider for Vec<Arc<dyn Tool>> {
    fn tools(&self) -> ToolsFuture<'_> {
        let tools = self.clone();
        Box::pin(async move { tools })
    }
}

/// Handler result: (content, is_error, trace_id).
pub type HandlerResult = Result<(Vec<Content>, bool, Option<String>), ToolCallError>;

/// Async function type for tool call handling (2-param).
pub type ToolCallFn = Arc<
    dyn Fn(String, serde_json::Value) -> Pin<Box<dyn Future<Output = HandlerResult> + Send>>
        + Send
        + Sync,
>;

/// Async function type for tool call handling (3-param, with request).
pub type ToolCallWithRequestFn = Arc<
    dyn Fn(
            String,
            serde_json::Value,
            Request,
        ) -> Pin<Box<dyn Future<Output = HandlerResult> + Send>>
        + Send
        + Sync,
>;

/// Tool call handler — either 2-param or 3-param.
#[derive(Clone)]
pub enum ToolCallHandler {
    /// Handler: (name, args) -> result.
    Basic(ToolCallFn),
    /// Handler: (name, args, request) -> result.
    WithRequest(ToolCallWithRequestFn),
}

/// Auth hook result.
pub type AuthResult = Result<(), AuthError>;

/// Async auth hook function. Takes owned request `Parts` for full request context.
///
/// Returns only a pass/fail verdict: it receives no continuation and hands
/// back no value, so it **cannot** propagate an authenticated identity. Use
/// [`Authenticator`] when the tool handler needs to know *who* the caller is —
/// see that trait's docs for why both exist.
pub type AuthHookFn = Arc<
    dyn Fn(axum::http::request::Parts) -> Pin<Box<dyn Future<Output = AuthResult> + Send>>
        + Send
        + Sync,
>;

/// Optional auth hook.
///
/// Validation only — see [`AuthHookFn`]. For identity propagation use
/// [`Authenticator`], which takes precedence when both are configured.
#[derive(Clone, Default)]
pub struct AuthHook(pub Option<AuthHookFn>);

/// Configuration for the embedded UI.
#[derive(Clone)]
pub struct UiConfig {
    pub allow_execute: bool,
    pub title: String,
    pub project_name: Option<String>,
    pub project_url: Option<String>,
    /// Validation-only auth hook. Ignored when `authenticator` is set.
    ///
    /// Marked legacy because it cannot propagate an identity (see
    /// [`AuthHookFn`]), not because it is deprecated — it remains the right
    /// choice for a pure pass/fail gate.
    pub auth_hook: AuthHook,
    /// Full authenticator that returns an [`Identity`] on success.
    ///
    /// When set, it takes precedence over `auth_hook`. The resolved identity
    /// is propagated to tool call handlers via the
    /// [`AUTH_IDENTITY`](crate::AUTH_IDENTITY) task-local.
    pub authenticator: Option<Arc<dyn Authenticator>>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            allow_execute: false,
            title: "MCP Tool Explorer".to_string(),
            project_name: None,
            project_url: None,
            auth_hook: AuthHook::default(),
            authenticator: None,
        }
    }
}

/// Error returned by auth hooks.
#[derive(Debug, thiserror::Error)]
#[error("Unauthorized")]
pub struct AuthError;

// -- Authenticator ------------------------------------------------------------

/// Authenticated identity attached to a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Unique identifier (user ID, service account name, etc).
    pub id: String,
    /// Type of identity: `"human"`, `"service"`, `"agent"`, etc.
    pub identity_type: String,
    /// Roles or permissions assigned to this identity.
    pub roles: Vec<String>,
    /// Arbitrary key-value attributes.
    pub attrs: HashMap<String, serde_json::Value>,
}

/// Trait for authenticating incoming requests by inspecting HTTP headers.
///
/// Implement this trait to plug apcore (or any other) authentication into the
/// embedded UI. The [`Identity`] returned is made available to tool call
/// handlers via the [`AUTH_IDENTITY`](crate::AUTH_IDENTITY) task-local.
///
/// # Why this exists alongside [`AuthHook`]
///
/// [`AuthHook`] is a *validation-only* gate. Its signature —
/// `Fn(Parts) -> Future<Output = Result<(), AuthError>>` — yields a pass/fail
/// verdict and nothing else, and it does **not** receive the continuation, so
/// it can neither hand an identity back to the library nor scope one around
/// the tool call. `server.rs` therefore records `identity = None` on that
/// path; identity propagation through `AuthHook` alone is structurally
/// impossible in Rust.
///
/// The sibling bindings do not have this problem, because their hook shapes
/// carry the call: the Python `AuthHook` returns a context manager and the
/// caller sets a `contextvars.ContextVar` across the `yield`; the TypeScript
/// one is `(req, next) => Promise<Response>` and the caller wraps `next` in
/// `AsyncLocalStorage.run`. Both propagate identity entirely in the caller,
/// exactly as F4 prescribes, and neither needs anything from this library.
///
/// Rust has no equivalent that a bare `AuthHook` could drive. Without
/// `Authenticator`, a Rust integration would have to expose a materially
/// different API to its users than the Python and TypeScript ones do —
/// "authenticate, then find your own way to reach the identity" instead of
/// "pass an authenticator and read the identity in your handler".
///
/// `Authenticator` closes that gap: the library performs the authentication
/// and scopes the resulting [`Identity`] into
/// [`AUTH_IDENTITY`](crate::AUTH_IDENTITY) for the duration of the call. The point is **caller-facing API parity across all
/// three language bindings**, not a richer feature set for Rust — the observed
/// behaviour is the same in every language; only the layer that implements it
/// differs, because the languages differ.
///
/// [`AuthHook`] is kept for callers that only need a pass/fail gate and have
/// no use for an identity. When both are set on [`UiConfig`],
/// `Authenticator` wins.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Inspect `headers` and return an [`Identity`] on success, or `None` to
    /// reject the request with `401 Unauthorized`.
    async fn authenticate(&self, headers: &HashMap<String, String>) -> Option<Identity>;
}

/// Error returned by tool call handlers.
#[derive(Debug, thiserror::Error)]
pub enum ToolCallError {
    #[error("{0}")]
    Internal(String),
}

/// JSON error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// A single normalized JSON Schema validation failure.
///
/// Wire format used by `POST /tools/{name}/validate`. `path` is a JSON Pointer
/// (RFC 6901) — empty string for the root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFailure {
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
}

/// Response envelope for `POST /tools/{name}/validate`.
///
/// `errors` is omitted from the JSON output entirely when validation passes,
/// per the F7 spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub errors: Vec<ValidationFailure>,
}

/// Dynamic tools provider backed by an async function.
pub struct DynamicToolsProvider<F>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Vec<Arc<dyn Tool>>> + Send>> + Send + Sync + 'static,
{
    provider: F,
}

impl<F> DynamicToolsProvider<F>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Vec<Arc<dyn Tool>>> + Send>> + Send + Sync + 'static,
{
    pub fn new(provider: F) -> Self {
        Self { provider }
    }
}

impl<F> ToolsProvider for DynamicToolsProvider<F>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Vec<Arc<dyn Tool>>> + Send>> + Send + Sync + 'static,
{
    fn tools(&self) -> ToolsFuture<'_> {
        (self.provider)()
    }
}
