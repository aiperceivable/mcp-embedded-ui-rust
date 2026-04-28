//! Tests for mcp-embedded-ui route handlers.
//!
//! Ported from the Python reference implementation test suite.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use mcp_embedded_ui::{
    AuthError, AuthHook, Content, DynamicToolsProvider, HandlerResult, Tool, ToolCallError,
    ToolCallHandler, ToolsProvider, UiConfig,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct FakeTool {
    tool_name: String,
    tool_description: String,
    tool_input_schema: serde_json::Value,
    tool_annotations: Option<serde_json::Value>,
}

impl FakeTool {
    fn new(name: &str, description: &str) -> Self {
        Self {
            tool_name: name.to_string(),
            tool_description: description.to_string(),
            tool_input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "msg": {"type": "string"}
                }
            }),
            tool_annotations: None,
        }
    }

    fn with_annotations(mut self, annotations: serde_json::Value) -> Self {
        self.tool_annotations = Some(annotations);
        self
    }
}

impl Tool for FakeTool {
    fn name(&self) -> &str {
        &self.tool_name
    }
    fn description(&self) -> &str {
        &self.tool_description
    }
    fn input_schema(&self) -> serde_json::Value {
        self.tool_input_schema.clone()
    }
    fn annotations(&self) -> Option<serde_json::Value> {
        self.tool_annotations.clone()
    }
}

fn fake_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(
            FakeTool::new("echo", "Echo back")
                .with_annotations(serde_json::json!({"readOnlyHint": true})),
        ),
        Arc::new(FakeTool::new("boom", "Always errors")),
    ]
}

fn fake_handler() -> ToolCallHandler {
    ToolCallHandler::Basic(Arc::new(
        |name: String,
         args: serde_json::Value|
         -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
            Box::pin(async move {
                match name.as_str() {
                    "echo" => {
                        let msg = args.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        Ok((
                            vec![Content {
                                content_type: "text".to_string(),
                                text: Some(format!("echo: {}", msg)),
                                mime_type: None,
                                data: None,
                            }],
                            false,
                            Some("t1".to_string()),
                        ))
                    }
                    "boom" => Ok((
                        vec![Content {
                            content_type: "text".to_string(),
                            text: Some("kaboom".to_string()),
                            mime_type: None,
                            data: None,
                        }],
                        true,
                        None,
                    )),
                    _ => Err(ToolCallError::Internal(format!("Unknown tool: {}", name))),
                }
            })
        },
    ))
}

fn build_router(config: UiConfig) -> axum::Router {
    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    mcp_embedded_ui::build_ui_routes(tools, fake_handler(), config)
}

fn default_config() -> UiConfig {
    UiConfig::default()
}

async fn send_get(router: &axum::Router, uri: &str) -> (StatusCode, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn send_post(router: &axum::Router, uri: &str, body_str: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body_str.to_string()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn send_post_with_headers(
    router: &axum::Router,
    uri: &str,
    body_str: &str,
    headers: Vec<(&str, &str)>,
) -> (StatusCode, String) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        builder = builder.header(k, v);
    }
    let req = builder.body(Body::from(body_str.to_string())).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn send_post_raw(
    router: &axum::Router,
    uri: &str,
    body_bytes: &[u8],
) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body_bytes.to_vec()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

// ---------------------------------------------------------------------------
// Explorer page
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explorer_page_returns_html() {
    let router = build_router(default_config());
    let (status, body) = send_get(&router, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("MCP Tool Explorer"));
}

#[tokio::test]
async fn test_explorer_page_custom_title() {
    let config = UiConfig {
        title: "My Custom Explorer".to_string(),
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(body.contains("My Custom Explorer"));
    assert!(!body.contains("{{TITLE}}"));
}

// ---------------------------------------------------------------------------
// Allow execute in HTML
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_default_execute_disabled_in_html() {
    let router = build_router(default_config());
    let (_, body) = send_get(&router, "/").await;
    assert!(body.contains("var executeEnabled = false;"));
}

#[tokio::test]
async fn test_execute_enabled_in_html() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(body.contains("var executeEnabled = true;"));
}

// ---------------------------------------------------------------------------
// Project link
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_project_link_by_default() {
    let router = build_router(default_config());
    let (_, body) = send_get(&router, "/").await;
    assert!(!body.contains("{{PROJECT_LINK}}"));
    assert!(!body.contains("&middot;"));
}

#[tokio::test]
async fn test_project_name_only() {
    let config = UiConfig {
        project_name: Some("my-project".to_string()),
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(body.contains("&middot; my-project"));
}

#[tokio::test]
async fn test_project_name_and_url() {
    let config = UiConfig {
        project_name: Some("my-project".to_string()),
        project_url: Some("https://github.com/example/my-project".to_string()),
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(body.contains("my-project"));
    assert!(body.contains("https://github.com/example/my-project"));
}

#[tokio::test]
async fn test_project_name_xss_escaped() {
    let config = UiConfig {
        project_name: Some("<script>alert(1)</script>".to_string()),
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(!body.contains("<script>alert(1)</script>"));
    assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
}

// ---------------------------------------------------------------------------
// List tools
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_tools_returns_all() {
    let router = build_router(default_config());
    let (status, body) = send_get(&router, "/tools").await;
    assert_eq!(status, StatusCode::OK);
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(data.len(), 2);
    let names: Vec<&str> = data.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"boom"));
}

#[tokio::test]
async fn test_annotations_included_when_present() {
    let router = build_router(default_config());
    let (_, body) = send_get(&router, "/tools").await;
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    let echo = data.iter().find(|t| t["name"] == "echo").unwrap();
    assert_eq!(echo["annotations"]["readOnlyHint"], true);
}

#[tokio::test]
async fn test_annotations_omitted_when_none() {
    let router = build_router(default_config());
    let (_, body) = send_get(&router, "/tools").await;
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    let boom = data.iter().find(|t| t["name"] == "boom").unwrap();
    assert!(boom.get("annotations").is_none());
}

// ---------------------------------------------------------------------------
// Tool detail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_detail_existing() {
    let router = build_router(default_config());
    let (status, body) = send_get(&router, "/tools/echo").await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["name"], "echo");
    assert!(data.get("inputSchema").is_some());
}

#[tokio::test]
async fn test_tool_detail_missing_returns_404() {
    let router = build_router(default_config());
    let (status, _) = send_get(&router, "/tools/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Call tool
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_call_tool_success() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (status, body) = send_post(&router, "/tools/echo/call", r#"{"msg":"hi"}"#).await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["isError"], false);
    assert_eq!(data["content"][0]["text"], "echo: hi");
    assert_eq!(data["_meta"]["_trace_id"], "t1");
}

#[tokio::test]
async fn test_call_tool_error() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (status, body) = send_post(&router, "/tools/boom/call", "{}").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["isError"], true);
}

#[tokio::test]
async fn test_call_tool_missing_returns_404() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (status, _) = send_post(&router, "/tools/nope/call", "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_call_tool_invalid_json_treated_as_empty() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (status, _) = send_post_raw(&router, "/tools/echo/call", b"not json").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Validate tool — F7
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SchemaTool {
    n: String,
    schema: serde_json::Value,
}

impl Tool for SchemaTool {
    fn name(&self) -> &str {
        &self.n
    }
    fn description(&self) -> &str {
        ""
    }
    fn input_schema(&self) -> serde_json::Value {
        self.schema.clone()
    }
}

fn validate_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(SchemaTool {
            n: "echo".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["city"]
            }),
        }),
        Arc::new(SchemaTool {
            n: "noschema".to_string(),
            schema: serde_json::json!({}),
        }),
    ]
}

fn build_validate_router(config: UiConfig) -> axum::Router {
    let tools: Arc<dyn ToolsProvider> = Arc::new(validate_tools());
    mcp_embedded_ui::build_ui_routes(tools, fake_handler(), config)
}

#[tokio::test]
async fn test_validate_valid_input() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(true));
    assert!(
        v.get("errors").is_none(),
        "errors key must be omitted on success, got: {body}"
    );
}

#[tokio::test]
async fn test_validate_missing_required_field() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post(&router, "/tools/echo/validate", "{}").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(false));
    let errors = v["errors"].as_array().unwrap();
    assert!(errors.iter().any(|e| e["keyword"] == "required"));
}

#[tokio::test]
async fn test_validate_wrong_type_with_path() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris", "count": "not-int"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(false));
    let errors = v["errors"].as_array().unwrap();
    let type_err = errors
        .iter()
        .find(|e| e["keyword"] == "type")
        .expect("expected at least one type error");
    assert_eq!(type_err["path"], "/count");
}

#[tokio::test]
async fn test_validate_multiple_errors() {
    let multi: Vec<Arc<dyn Tool>> = vec![Arc::new(SchemaTool {
        n: "multi".to_string(),
        schema: serde_json::json!({
            "type": "object",
            "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}},
            "required": ["a", "b"]
        }),
    })];
    let tools: Arc<dyn ToolsProvider> = Arc::new(multi);
    let router = mcp_embedded_ui::build_ui_routes(tools, fake_handler(), default_config());
    let (status, body) = send_post(&router, "/tools/multi/validate", "{}").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["errors"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn test_validate_tool_not_found() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post(&router, "/tools/nope/validate", "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["error"].as_str().unwrap().contains("Tool not found"));
}

#[tokio::test]
async fn test_validate_invalid_json_returns_400() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post_raw(&router, "/tools/echo/validate", b"not json").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(false));
    assert_eq!(v["errors"][0]["keyword"], "format");
    assert_eq!(v["errors"][0]["path"], "");
}

#[tokio::test]
async fn test_validate_no_schema_always_valid() {
    let router = build_validate_router(default_config());
    let (status, body) = send_post(
        &router,
        "/tools/noschema/validate",
        r#"{"anything": 123}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(true));
}

#[tokio::test]
async fn test_validate_ignores_allow_execute_false() {
    let router = build_validate_router(default_config()); // allow_execute=false default
    let (status, body) = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["valid"], serde_json::Value::Bool(true));
}

#[tokio::test]
async fn test_validate_does_not_invoke_auth_hook() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_inner = counter.clone();
    let auth_hook_fn: mcp_embedded_ui::AuthHookFn = Arc::new(move |_parts| {
        let c = counter_inner.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<(), AuthError>(AuthError)
        })
            as Pin<Box<dyn Future<Output = Result<(), AuthError>> + Send>>
    });
    let config = UiConfig {
        allow_execute: true,
        auth_hook: AuthHook(Some(auth_hook_fn)),
        ..default_config()
    };
    let router = build_validate_router(config);
    let (status, _) = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "auth_hook must not be invoked for /validate"
    );
}

#[tokio::test]
async fn test_validate_does_not_invoke_handler() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_inner = counter.clone();

    let handler = ToolCallHandler::Basic(Arc::new(
        move |_name: String, _args: serde_json::Value| {
            let c = counter_inner.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok((Vec::<Content>::new(), false, None))
            }) as Pin<Box<dyn Future<Output = HandlerResult> + Send>>
        },
    ));

    let tools: Arc<dyn ToolsProvider> = Arc::new(validate_tools());
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = mcp_embedded_ui::build_ui_routes(tools, handler, config);
    let _ = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris"}"#,
    )
    .await;
    let _ = send_post(&router, "/tools/echo/validate", "{}").await;
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "handle_call must never be invoked from /validate"
    );
}

#[tokio::test]
async fn test_validate_omits_errors_when_valid() {
    let router = build_validate_router(default_config());
    let (_, body) = send_post(
        &router,
        "/tools/echo/validate",
        r#"{"city": "Paris"}"#,
    )
    .await;
    // Raw body must not contain the "errors" key at all.
    assert!(
        !body.contains("\"errors\""),
        "body should not contain `errors` key when valid, got: {body}"
    );
}

// ---------------------------------------------------------------------------
// allow_execute=false
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execution_disabled_returns_403() {
    let router = build_router(default_config());
    let (status, _) = send_post(&router, "/tools/echo/call", "{}").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_and_detail_still_work_when_disabled() {
    let router = build_router(default_config());
    let (status, _) = send_get(&router, "/tools").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_get(&router, "/tools/echo").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Auth hook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_valid_auth_passes() {
    let config = UiConfig {
        allow_execute: true,
        auth_hook: AuthHook(Some(Arc::new(|parts| {
            Box::pin(async move {
                let auth = parts
                    .headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if auth.contains("valid") {
                    Ok(())
                } else {
                    Err(AuthError)
                }
            })
        }))),
        ..default_config()
    };
    let router = build_router(config);
    let (status, _) = send_post_with_headers(
        &router,
        "/tools/echo/call",
        r#"{"msg":"hi"}"#,
        vec![("authorization", "Bearer valid-token")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_auth_returns_401() {
    let config = UiConfig {
        allow_execute: true,
        auth_hook: AuthHook(Some(Arc::new(|_parts| {
            Box::pin(async move { Err(AuthError) })
        }))),
        ..default_config()
    };
    let router = build_router(config);
    let (status, body) = send_post(&router, "/tools/echo/call", "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["error"], "Unauthorized");
}

// ---------------------------------------------------------------------------
// Trace ID
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_meta_when_trace_id_is_none() {
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_post(&router, "/tools/boom/call", "{}").await;
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(data.get("_meta").is_none());
}

// ---------------------------------------------------------------------------
// Dynamic tools
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dynamic_tools_provider() {
    let provider = DynamicToolsProvider::new(|| {
        Box::pin(async {
            vec![Arc::new(FakeTool::new("dynamic", "A dynamic tool")) as Arc<dyn Tool>]
        })
    });
    let tools: Arc<dyn ToolsProvider> = Arc::new(provider);
    let router = mcp_embedded_ui::build_ui_routes(tools, fake_handler(), default_config());

    let (_, body) = send_get(&router, "/tools").await;
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "dynamic");
}

// ---------------------------------------------------------------------------
// XSS prevention in title
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_script_tag_in_title_is_escaped() {
    let config = UiConfig {
        title: "<script>alert(\"xss\")</script>".to_string(),
        ..default_config()
    };
    let router = build_router(config);
    let (_, body) = send_get(&router, "/").await;
    assert!(!body.contains("<script>alert"));
    assert!(body.contains("&lt;script&gt;"));
}

// ---------------------------------------------------------------------------
// Handler exception
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_handler_exception_returns_500() {
    let throwing_handler = ToolCallHandler::Basic(Arc::new(
        |_name: String,
         _args: serde_json::Value|
         -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
            Box::pin(async move { Err(ToolCallError::Internal("internal failure".to_string())) })
        },
    ));

    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = mcp_embedded_ui::build_ui_routes(tools, throwing_handler, config);

    let (status, body) = send_post(&router, "/tools/echo/call", r#"{"msg":"hi"}"#).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["isError"], true);
    assert_eq!(data["content"][0]["type"], "text");
    assert!(data["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("internal failure"));
}

// ---------------------------------------------------------------------------
// GET endpoints do not invoke auth hook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_endpoints_do_not_invoke_auth_hook() {
    let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count_clone = Arc::clone(&call_count);

    let config = UiConfig {
        allow_execute: true,
        auth_hook: AuthHook(Some(Arc::new(move |_parts| {
            count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move { Err(AuthError) })
        }))),
        ..default_config()
    };
    let router = build_router(config);

    let (status, _) = send_get(&router, "/").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_get(&router, "/tools").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_get(&router, "/tools/echo").await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        call_count.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Auth hook was called on GET endpoints"
    );
}

// ---------------------------------------------------------------------------
// Auth error does not leak internals
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auth_error_does_not_leak_details() {
    let config = UiConfig {
        allow_execute: true,
        auth_hook: AuthHook(Some(Arc::new(|_parts| {
            Box::pin(async move { Err(AuthError) })
        }))),
        ..default_config()
    };
    let router = build_router(config);
    let (status, body) = send_post(&router, "/tools/echo/call", "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["error"], "Unauthorized");
    assert!(data.get("detail").is_none());
}

// ---------------------------------------------------------------------------
// WithRequest handler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_call_tool_with_request_handler() {
    let handler = ToolCallHandler::WithRequest(Arc::new(
        |name: String,
         args: serde_json::Value,
         req: axum::extract::Request|
         -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
            Box::pin(async move {
                let custom = req
                    .headers()
                    .get("x-custom-header")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("missing")
                    .to_string();
                let msg = args.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                Ok((
                    vec![Content {
                        content_type: "text".to_string(),
                        text: Some(format!("{}: {} (header={})", name, msg, custom)),
                        mime_type: None,
                        data: None,
                    }],
                    false,
                    None,
                ))
            })
        },
    ));

    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = mcp_embedded_ui::build_ui_routes(tools, handler, config);

    let (status, body) = send_post_with_headers(
        &router,
        "/tools/echo/call",
        r#"{"msg":"hi"}"#,
        vec![("x-custom-header", "test-value")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["isError"], false);
    let text = data["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("header=test-value"),
        "Expected header value in response, got: {}",
        text
    );
}

#[tokio::test]
async fn test_with_request_handler_receives_method_and_uri() {
    let handler = ToolCallHandler::WithRequest(Arc::new(
        |_name: String,
         _args: serde_json::Value,
         req: axum::extract::Request|
         -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
            Box::pin(async move {
                let method = req.method().to_string();
                let uri = req.uri().to_string();
                Ok((
                    vec![Content {
                        content_type: "text".to_string(),
                        text: Some(format!("method={} uri={}", method, uri)),
                        mime_type: None,
                        data: None,
                    }],
                    false,
                    None,
                ))
            })
        },
    ));

    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = mcp_embedded_ui::build_ui_routes(tools, handler, config);

    let (status, body) = send_post(&router, "/tools/echo/call", "{}").await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    let text = data["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("method=POST"),
        "Expected POST method, got: {}",
        text
    );
    assert!(
        text.contains("uri=/tools/echo/call"),
        "Expected correct URI, got: {}",
        text
    );
}

// ---------------------------------------------------------------------------
// HTML template placeholder validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_raw_placeholder_in_html() {
    let router = build_router(default_config());
    let (_, body) = send_get(&router, "/").await;
    assert!(!body.contains("{{TITLE}}"));
    assert!(!body.contains("{{ALLOW_EXECUTE}}"));
    assert!(!body.contains("{{PROJECT_LINK}}"));
}

// ---------------------------------------------------------------------------
// create_mount
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_mount_default_prefix() {
    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let router = mcp_embedded_ui::create_mount(None, tools, fake_handler(), default_config());

    let (status, body) = send_get(&router, "/explorer").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("MCP Tool Explorer"));

    let (status, body) = send_get(&router, "/explorer/tools").await;
    assert_eq!(status, StatusCode::OK);
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn test_create_mount_custom_prefix() {
    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let router =
        mcp_embedded_ui::create_mount(Some("/ui"), tools, fake_handler(), default_config());

    let (status, body) = send_get(&router, "/ui").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("MCP Tool Explorer"));

    let (status, body) = send_get(&router, "/ui/tools").await;
    assert_eq!(status, StatusCode::OK);
    let data: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn test_create_mount_call_tool_works() {
    let tools: Arc<dyn ToolsProvider> = Arc::new(fake_tools());
    let config = UiConfig {
        allow_execute: true,
        ..default_config()
    };
    let router = mcp_embedded_ui::create_mount(None, tools, fake_handler(), config);

    let (status, body) = send_post(&router, "/explorer/tools/echo/call", r#"{"msg":"hi"}"#).await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["content"][0]["text"], "echo: hi");
}

// ---------------------------------------------------------------------------
// HTML template drift check — spec repo vs implementation
// ---------------------------------------------------------------------------

#[test]
fn test_template_matches_spec_repo() {
    let pkg_html = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/explorer.html");
    let spec_html = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mcp-embedded-ui/docs/explorer.html");

    if !spec_html.exists() {
        return;
    }

    let pkg_content = std::fs::read_to_string(&pkg_html).expect("failed to read pkg html");
    let spec_content = std::fs::read_to_string(&spec_html).expect("failed to read spec html");

    assert_eq!(
        pkg_content, spec_content,
        "explorer.html has drifted from spec repo. \
         Run: cp ../mcp-embedded-ui/docs/explorer.html src/explorer.html"
    );
}
