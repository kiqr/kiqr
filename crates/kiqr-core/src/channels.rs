//! Channels: where a project's content goes out. A channel is a connection
//! (`connections.rs`) whose ability publishes: it names a kind and a
//! destination (an address, a subreddit, a blog) and carries its own account.
//! Publishing always asks the user first; the channel only remembers where.
//! Here: what publishing needs besides the channel itself.

pub use crate::connections::{new_id, valid_id, Connection as Channel};

/// Markdown to simple HTML for email bodies and blog posts.
pub fn markdown_to_html(md: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    let parser = pulldown_cmark::Parser::new_ext(md, opts);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_renders_and_ids_are_safe() {
        let h = markdown_to_html("# Hi\n\nOne *two* [three](https://x.y).\n\n- a\n- b\n");
        assert!(h.contains("<h1>Hi</h1>") && h.contains("<em>two</em>") && h.contains("<li>a</li>"));
        assert!(valid_id("email-to-jane") && !valid_id("Email") && !valid_id("-x"));
        let c = Channel { destination: [("to".to_string(), serde_json::json!("jane@example.com"))].into_iter().collect(), ..Default::default() };
        assert_eq!(c.destination_text(), "jane@example.com");
    }
}

/// Tag links to the business's own site with UTM parameters, so analytics
/// can tell what KIQR published and for which campaign. Other links are left
/// alone; links that already carry a `utm_` parameter too.
pub fn add_utm(text: &str, site_host: &str, source: &str, campaign: Option<&str>) -> String {
    if site_host.is_empty() {
        return text.to_string();
    }
    let host = site_host.trim_start_matches("www.").to_lowercase();
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    while let Some(i) = rest.find("http") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '<' | '>' | ']')).unwrap_or(tail.len());
        let (link, after) = tail.split_at(end);
        let link_trim = link.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let punct = &link[link_trim.len()..];
        match url::Url::parse(link_trim) {
            Ok(u) if u.host_str().map(|h| h.trim_start_matches("www.").to_lowercase()) == Some(host.clone()) && !u.query().unwrap_or("").contains("utm_") => {
                let mut u = u;
                {
                    let mut q = u.query_pairs_mut();
                    q.append_pair("utm_source", source);
                    q.append_pair("utm_medium", "kiqr");
                    if let Some(c) = campaign.filter(|c| !c.is_empty()) {
                        q.append_pair("utm_campaign", c);
                    }
                }
                out.push_str(u.as_str());
                out.push_str(punct);
            }
            _ => out.push_str(link),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod utm_tests {
    use super::add_utm;

    #[test]
    fn tags_own_links_only() {
        let t = "See https://www.example.com/pricing. Not https://other.com/x and not https://example.com/a?utm_source=x";
        let r = add_utm(t, "example.com", "linkedin", Some("launch"));
        assert!(r.contains("https://www.example.com/pricing?utm_source=linkedin&utm_medium=kiqr&utm_campaign=launch."));
        assert!(r.contains("https://other.com/x "));
        assert!(r.contains("https://example.com/a?utm_source=x"));
        assert_eq!(add_utm("no links", "example.com", "x", None), "no links");
    }
}
