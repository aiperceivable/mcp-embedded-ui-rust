//! A lightweight embedded Web UI for any MCP Server.
//!
//! Provides Axum routes that serve a self-contained HTML explorer page and
//! JSON API endpoints for listing, inspecting, and executing MCP tools.

mod html;
mod server;
mod types;

#[allow(deprecated)]
pub use server::{build_mcp_ui_routes, build_ui_routes, create_app, create_mount};
pub use types::{
    AuthError, AuthHook, AuthHookFn, AuthResult, CallResult, CallResultMeta, Content,
    DynamicToolsProvider, ErrorResponse, HandlerResult, Tool, ToolCallError, ToolCallHandler,
    ToolDetail, ToolSummary, ToolsFuture, ToolsProvider, UiConfig,
};
