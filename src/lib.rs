//! A lightweight embedded Web UI for any MCP Server.
//!
//! Provides Axum routes that serve a self-contained HTML explorer page and
//! JSON API endpoints for listing, inspecting, and executing MCP tools.

mod html;
mod server;
mod types;

#[allow(deprecated)]
pub use server::{build_mcp_ui_routes, build_ui_routes, create_app, create_mount, AUTH_IDENTITY};
pub use types::{
    AuthError, AuthHook, AuthHookFn, AuthResult, Authenticator, CallResult, CallResultMeta,
    Content, DynamicToolsProvider, ErrorResponse, HandlerResult, Identity, Tool, ToolCallError,
    ToolCallFn, ToolCallHandler, ToolCallWithRequestFn, ToolDetail, ToolSummary, ToolsFuture,
    ToolsProvider, UiConfig,
};
