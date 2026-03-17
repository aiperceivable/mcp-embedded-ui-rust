//! Core route generation and Axum router factory for mcp-embedded-ui.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;

use crate::html::render_explorer_html;
use crate::types::{
    CallResult, CallResultMeta, Content, ErrorResponse, Identity, ToolCallHandler, ToolDetail,
    ToolSummary, ToolsProvider, UiConfig,
};

tokio::task_local! {
    /// Authenticated identity for the current tool call.
    ///
    /// Set to `Some(identity)` when an [`Authenticator`](crate::Authenticator) is configured
    /// and authentication succeeds. Tool call handlers can read it via:
    ///
    /// ```rust,ignore
    /// use mcp_embedded_ui::AUTH_IDENTITY;
    ///
    /// let id = AUTH_IDENTITY.try_with(|id| id.clone()).ok().flatten();
    /// ```
    pub static AUTH_IDENTITY: Option<Identity>;
}

/// Shared state for all route handlers.
struct AppState {
    tools: Arc<dyn ToolsProvider>,
    handler: ToolCallHandler,
    config: UiConfig,
    html_page: String,
}

/// Build an Axum `Router` for the MCP Embedded UI.
///
/// This is the primary entry point. The returned router can be nested
/// under a prefix in any Axum application.
pub fn build_ui_routes(
    tools: Arc<dyn ToolsProvider>,
    handler: ToolCallHandler,
    config: UiConfig,
) -> Router {
    let html_page = render_explorer_html(
        Some(&config.title),
        config.allow_execute,
        config.project_name.as_deref(),
        config.project_url.as_deref(),
    );

    let state = Arc::new(AppState {
        tools,
        handler,
        config,
        html_page,
    });

    Router::new()
        .route("/", get(explorer_page))
        .route("/tools", get(list_tools))
        .route("/tools/{name}", get(tool_detail))
        .route("/tools/{name}/call", axum::routing::post(call_tool))
        .with_state(state)
}

/// Create a standalone Axum `Router` (convenience wrapper).
pub fn create_app(
    tools: Arc<dyn ToolsProvider>,
    handler: ToolCallHandler,
    config: UiConfig,
) -> Router {
    build_ui_routes(tools, handler, config)
}

/// Create an Axum `Router` with the UI nested under a URL prefix.
///
/// Defaults to `"/explorer"` when `prefix` is `None`.
pub fn create_mount(
    prefix: Option<&str>,
    tools: Arc<dyn ToolsProvider>,
    handler: ToolCallHandler,
    config: UiConfig,
) -> Router {
    let prefix = prefix.unwrap_or("/explorer");
    Router::new().nest(prefix, build_ui_routes(tools, handler, config))
}

// -- Backward compatibility ---------------------------------------------------

/// Deprecated: use [`build_ui_routes`] instead.
#[deprecated(note = "build_mcp_ui_routes is deprecated, use build_ui_routes instead")]
pub fn build_mcp_ui_routes(
    tools: Arc<dyn ToolsProvider>,
    handler: ToolCallHandler,
    config: UiConfig,
) -> Router {
    build_ui_routes(tools, handler, config)
}

// -- Handlers -----------------------------------------------------------------

async fn explorer_page(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(state.html_page.clone())
}

async fn list_tools(State(state): State<Arc<AppState>>) -> Json<Vec<ToolSummary>> {
    let tools = state.tools.tools().await;
    let summaries: Vec<ToolSummary> = tools
        .iter()
        .map(|t| ToolSummary {
            name: t.name().to_string(),
            description: t.description().to_string(),
            annotations: t.annotations(),
        })
        .collect();
    Json(summaries)
}

async fn tool_detail(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    let tools = state.tools.tools().await;
    let tool = tools.iter().find(|t| t.name() == name);

    match tool {
        Some(t) => {
            let detail = ToolDetail {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
                annotations: t.annotations(),
            };
            Json(detail).into_response()
        }
        None => {
            let body = ErrorResponse {
                error: format!("Tool not found: {}", name),
            };
            (StatusCode::NOT_FOUND, Json(body)).into_response()
        }
    }
}

async fn call_tool(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();

    // Precondition 1: allow_execute
    if !state.config.allow_execute {
        let resp = ErrorResponse {
            error: "Tool execution is disabled.".to_string(),
        };
        return (StatusCode::FORBIDDEN, Json(resp)).into_response();
    }

    // Precondition 2: tool exists
    let tools = state.tools.tools().await;
    let tool = tools.iter().find(|t| t.name() == name);
    if tool.is_none() {
        let resp = ErrorResponse {
            error: format!("Tool not found: {}", name),
        };
        return (StatusCode::NOT_FOUND, Json(resp)).into_response();
    }

    // Precondition 3: read body and parse (invalid JSON → empty object)
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => axum::body::Bytes::new(),
    };
    let body_str = String::from_utf8_lossy(&body_bytes);
    let args: serde_json::Value = serde_json::from_str(&body_str)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Precondition 4: authentication
    // Authenticator (full identity) takes precedence over legacy auth_hook.
    let identity: Option<Identity> = if let Some(ref authenticator) = state.config.authenticator {
        let headers = extract_headers(&parts);
        match authenticator.authenticate(&headers).await {
            Some(id) => Some(id),
            None => {
                tracing::warn!(tool = %name, "Authenticator rejected request");
                let resp = ErrorResponse {
                    error: "Unauthorized".to_string(),
                };
                return (StatusCode::UNAUTHORIZED, Json(resp)).into_response();
            }
        }
    } else if let Some(ref auth_fn) = state.config.auth_hook.0 {
        let auth_result = auth_fn(parts.clone()).await;
        if auth_result.is_err() {
            tracing::warn!(tool = %name, "Auth hook rejected request");
            let resp = ErrorResponse {
                error: "Unauthorized".to_string(),
            };
            return (StatusCode::UNAUTHORIZED, Json(resp)).into_response();
        }
        None
    } else {
        None
    };

    AUTH_IDENTITY
        .scope(identity, do_call(&state, &name, args, parts))
        .await
}

fn extract_headers(parts: &axum::http::request::Parts) -> HashMap<String, String> {
    parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect()
}

async fn do_call(
    state: &AppState,
    name: &str,
    args: serde_json::Value,
    parts: axum::http::request::Parts,
) -> Response {
    let result = match &state.handler {
        ToolCallHandler::Basic(f) => f(name.to_string(), args).await,
        ToolCallHandler::WithRequest(f) => {
            let req = Request::from_parts(parts, axum::body::Body::empty());
            f(name.to_string(), args, req).await
        }
    };

    match result {
        Ok((content, is_error, trace_id)) => {
            let meta = trace_id
                .filter(|id| !id.is_empty())
                .map(|id| CallResultMeta { trace_id: id });
            let call_result = CallResult {
                content,
                is_error,
                meta,
            };
            let status = if is_error {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::OK
            };
            (status, Json(call_result)).into_response()
        }
        Err(err) => {
            tracing::error!(tool = name, error = %err, "Tool call failed");
            let call_result = CallResult {
                content: vec![Content {
                    content_type: "text".to_string(),
                    text: Some(err.to_string()),
                    mime_type: None,
                    data: None,
                }],
                is_error: true,
                meta: None,
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(call_result)).into_response()
        }
    }
}
