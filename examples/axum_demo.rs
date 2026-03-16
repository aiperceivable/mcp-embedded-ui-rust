//! Demo Axum application showing mcp-embedded-ui usage.
//!
//! Run with: `cargo run --example axum_demo`
//! Then visit: http://localhost:8000/explorer

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use mcp_embedded_ui::{
    create_mount, AuthError, AuthHook, Content, HandlerResult, Tool, ToolCallError,
    ToolCallHandler, ToolsProvider, UiConfig,
};

// -- Mock MCP tools -----------------------------------------------------------

struct MockTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn input_schema(&self) -> serde_json::Value {
        self.input_schema.clone()
    }
}

fn demo_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(MockTool {
            name: "echo".to_string(),
            description: "Replies back with your message".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                }
            }),
        }),
        Arc::new(MockTool {
            name: "add".to_string(),
            description: "Add two numbers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                }
            }),
        }),
    ]
}

// -- Handler ------------------------------------------------------------------

fn demo_handler() -> ToolCallHandler {
    ToolCallHandler::Basic(Arc::new(
        |name: String,
         args: serde_json::Value|
         -> Pin<Box<dyn Future<Output = HandlerResult> + Send>> {
            Box::pin(async move {
                match name.as_str() {
                    "echo" => {
                        let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        Ok((
                            vec![Content {
                                content_type: "text".to_string(),
                                text: Some(format!("You said: {}", msg)),
                                mime_type: None,
                                data: None,
                            }],
                            false,
                            Some("trace-123".to_string()),
                        ))
                    }
                    "add" => {
                        let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        Ok((
                            vec![Content {
                                content_type: "text".to_string(),
                                text: Some(format!("Result: {}", a + b)),
                                mime_type: None,
                                data: None,
                            }],
                            false,
                            Some("trace-456".to_string()),
                        ))
                    }
                    _ => Err(ToolCallError::Internal(format!("Unknown tool: {}", name))),
                }
            })
        },
    ))
}

// -- Main ---------------------------------------------------------------------

const DEMO_TOKEN: &str = "demo-secret-token";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let tools: Arc<dyn ToolsProvider> = Arc::new(demo_tools());

    let config = UiConfig {
        allow_execute: true,
        title: "My MCP Explorer".to_string(),
        auth_hook: AuthHook(Some(Arc::new(|parts| {
            Box::pin(async move {
                let auth = parts
                    .headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if auth == format!("Bearer {}", DEMO_TOKEN) {
                    Ok(())
                } else {
                    Err(AuthError)
                }
            })
        }))),
        ..UiConfig::default()
    };

    let app = create_mount(None, tools, demo_handler(), config);

    println!("Running MCP Embedded UI at http://localhost:8000/explorer");
    println!("Auth token for demo: Bearer {}", DEMO_TOKEN);
    println!("Paste the token above into the UI's token field to execute tools");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
