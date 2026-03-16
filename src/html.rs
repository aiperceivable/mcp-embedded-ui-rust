//! Self-contained HTML page for the MCP Tool Explorer.

const EXPLORER_HTML_TEMPLATE: &str = include_str!("explorer.html");
const DEFAULT_TITLE: &str = "MCP Tool Explorer";

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn build_project_link(project_name: Option<&str>, project_url: Option<&str>) -> String {
    match (project_name, project_url) {
        (None, None) => String::new(),
        (Some(name), Some(url)) => {
            let escaped_name = html_escape(name);
            let escaped_url = html_escape(url);
            format!(
                " &middot; <a href=\"{}\" style=\"color:#888;text-decoration:none\" \
                 target=\"_blank\" rel=\"noopener\">{}</a>",
                escaped_url, escaped_name,
            )
        }
        (Some(name), None) => {
            let escaped_name = html_escape(name);
            format!(" &middot; {}", escaped_name)
        }
        (None, Some(_)) => String::new(),
    }
}

/// Render the explorer HTML page with the given configuration.
pub fn render_explorer_html(
    title: Option<&str>,
    allow_execute: bool,
    project_name: Option<&str>,
    project_url: Option<&str>,
) -> String {
    let title = title.unwrap_or(DEFAULT_TITLE);
    let escaped_title = html_escape(title);
    let execute_str = if allow_execute { "true" } else { "false" };
    let project_link = build_project_link(project_name, project_url);

    EXPLORER_HTML_TEMPLATE
        .replace("{{TITLE}}", &escaped_title)
        .replace("{{ALLOW_EXECUTE}}", execute_str)
        .replace("{{PROJECT_LINK}}", &project_link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape_basic() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_render_default_title() {
        let html = render_explorer_html(None, false, None, None);
        assert!(html.contains("MCP Tool Explorer"));
        assert!(!html.contains("{{TITLE}}"));
    }

    #[test]
    fn test_render_custom_title() {
        let html = render_explorer_html(Some("Custom"), false, None, None);
        assert!(html.contains("Custom"));
        assert!(!html.contains("{{TITLE}}"));
    }

    #[test]
    fn test_render_xss_in_title() {
        let html = render_explorer_html(Some("<script>alert(1)</script>"), false, None, None);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_allow_execute_false() {
        let html = render_explorer_html(None, false, None, None);
        assert!(html.contains("var executeEnabled = false;"));
    }

    #[test]
    fn test_allow_execute_true() {
        let html = render_explorer_html(None, true, None, None);
        assert!(html.contains("var executeEnabled = true;"));
    }

    #[test]
    fn test_project_link_none() {
        let html = render_explorer_html(None, false, None, None);
        assert!(!html.contains("&middot;"));
        assert!(!html.contains("{{PROJECT_LINK}}"));
    }

    #[test]
    fn test_project_link_name_only() {
        let html = render_explorer_html(None, false, Some("my-project"), None);
        assert!(html.contains("&middot; my-project"));
    }

    #[test]
    fn test_project_link_name_and_url() {
        let html =
            render_explorer_html(None, false, Some("my-project"), Some("https://example.com"));
        assert!(html.contains("my-project"));
        assert!(html.contains("https://example.com"));
    }

    #[test]
    fn test_project_name_xss_escaped() {
        let html = render_explorer_html(None, false, Some("<script>alert(1)</script>"), None);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }
}
