//! HTML → LLM-friendly Markdown, plus the bits of page structure the crawler
//! needs (title, canonical, links).

use scraper::{Html, Selector};
use url::Url;

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub title: String,
    pub canonical: Option<Url>,
    pub description: Option<String>,
    pub links: Vec<Url>,
    pub markdown: String,
    /// Visible text length after conversion; used to spot JavaScript shells.
    pub text_len: usize,
    pub script_len: usize,
}

pub fn parse(base: &Url, html: &str) -> Page {
    let doc = Html::parse_document(html);
    let sel = |s: &str| Selector::parse(s).expect("static selector");

    let title = doc
        .select(&sel("title"))
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let canonical = doc
        .select(&sel("link[rel=canonical]"))
        .next()
        .and_then(|l| l.value().attr("href"))
        .and_then(|h| base.join(h).ok());

    let description = doc
        .select(&sel("meta[name=description]"))
        .next()
        .and_then(|m| m.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut links = Vec::new();
    for a in doc.select(&sel("a[href]")) {
        if let Some(href) = a.value().attr("href") {
            if href.starts_with('#') || href.starts_with("mailto:") || href.starts_with("tel:") || href.starts_with("javascript:") {
                continue;
            }
            if let Ok(mut u) = base.join(href) {
                u.set_fragment(None);
                links.push(u);
            }
        }
    }

    let script_len: usize = doc.select(&sel("script")).map(|s| s.inner_html().len()).sum();

    // Strip chrome and scripts before conversion so the model reads content, not navigation.
    let cleaned = strip_nodes(html, &["head", "script", "style", "noscript", "svg", "nav", "footer", "header", "iframe", "form"]);
    let markdown = htmd::convert(&cleaned).unwrap_or_default();
    let markdown = squeeze_blank_lines(&markdown);
    let text_len = markdown.chars().filter(|c| !c.is_whitespace()).count();

    Page { title, canonical, description, links, markdown, text_len, script_len }
}

/// Remove whole elements by tag using the parsed tree, re-serialised.
fn strip_nodes(html: &str, tags: &[&str]) -> String {
    use scraper::node::Node;
    let doc = Html::parse_document(html);
    let mut out = String::with_capacity(html.len());
    fn walk(out: &mut String, node: ego_tree::NodeRef<Node>, tags: &[&str]) {
        match node.value() {
            Node::Element(el) => {
                let name = el.name();
                if tags.contains(&name) {
                    return;
                }
                out.push('<');
                out.push_str(name);
                for (k, v) in el.attrs() {
                    if k == "href" || k == "src" || k == "alt" || k == "title" {
                        out.push(' ');
                        out.push_str(k);
                        out.push_str("=\"");
                        out.push_str(&v.replace('"', "&quot;"));
                        out.push('"');
                    }
                }
                out.push('>');
                for child in node.children() {
                    walk(out, child, tags);
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
            Node::Text(t) => {
                let s: &str = t;
                out.push_str(&s.replace('<', "&lt;").replace('>', "&gt;"));
            }
            _ => {
                for child in node.children() {
                    walk(out, child, tags);
                }
            }
        }
    }
    walk(&mut out, doc.tree.root(), tags);
    out
}

fn squeeze_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.lines() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(t);
        out.push('\n');
    }
    out.trim().to_string()
}
