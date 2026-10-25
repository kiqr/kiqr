//! `transport: smtp`: mail through a server the manifest's `smtp` block
//! names (templates, usually `${VAR}` from `.env`). Serves the `email`
//! ability and email channels alike: `to`, `subject`, `text` (Markdown, sent
//! as plain text and HTML) and the asset's files attached.

use crate::ext::ExecOut;
use crate::App;
use anyhow::{anyhow, Context, Result};
use kiqr_core::extensions::{fill, Manifest};
use serde_json::{json, Value};

/// Send one call's mail. `args`: `to`, `subject` (else `title`), `text`, and
/// `asset` (`{type, id}`) whose files go along.
pub async fn exec(app: &App, slug: &str, m: &Manifest, args: &Value, env: &(impl Fn(&str) -> Option<String> + Clone)) -> Result<ExecOut> {
    let spec = m.smtp.as_ref().ok_or_else(|| anyhow!("{} has no smtp block", m.name))?;
    let get = |t: &str| -> Option<String> {
        let v = fill(t, &Value::Null, env.clone());
        let v = v.trim();
        (!v.is_empty() && !v.contains("${")).then(|| v.to_string())
    };
    let to = args["to"].as_str().unwrap_or("").trim().to_string();
    if to.is_empty() {
        anyhow::bail!("no address to send to");
    }
    let subject = args["subject"].as_str().filter(|s| !s.trim().is_empty()).or(args["title"].as_str()).unwrap_or("").to_string();
    let text = args["text"].as_str().unwrap_or("").to_string();
    let files = match (args["asset"]["type"].as_str(), args["asset"]["id"].as_str()) {
        (Some(k), Some(i)) if !i.is_empty() => {
            let a = kiqr_core::assets::load(&app.store, slug, k, i)?;
            crate::publish::asset_parts(app, slug, &a)?.1
        }
        _ => vec![],
    };
    let settings = Settings {
        host: get(&spec.host).ok_or_else(|| anyhow!("{} needs the mail server; set it under Connect", m.name))?,
        port: get(&spec.port).and_then(|p| p.parse().ok()),
        user: get(&spec.user),
        password: get(&spec.password),
        from: get(&spec.from),
        security: get(&spec.security),
    };
    let from = send(&settings, &to, &subject, &text, files).await?;
    Ok(ExecOut { json: json!({ "sent": true, "to": to, "from": from, "subject": subject }), text: Some(format!("sent to {to}")), file: None })
}

pub struct Settings {
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub from: Option<String>,
    pub security: Option<String>,
}

/// Send one email: Markdown text as both plain text and HTML, files
/// attached. Returns the From address used.
pub async fn send(s: &Settings, to: &str, subject: &str, text_md: &str, files: Vec<(String, Vec<u8>, String)>) -> Result<String> {
    use lettre::message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    let security = s.security.as_deref().map(str::to_ascii_lowercase).unwrap_or_else(|| if s.port == Some(465) { "tls".into() } else { "starttls".into() });
    let port = s.port.unwrap_or(if security == "tls" { 465 } else { 587 });
    let from_s = s.from.clone().or_else(|| s.user.clone()).ok_or_else(|| anyhow!("no From address: set the user name or the From address"))?;
    let from: Mailbox = from_s.parse().with_context(|| format!("'{from_s}' is not an address to send from"))?;
    let mut builder = Message::builder().from(from.clone()).subject(subject);
    for addr in to.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
        let mb: Mailbox = addr.parse().with_context(|| format!("'{addr}' is not an email address"))?;
        builder = builder.to(mb);
    }
    let html = format!("<div style=\"font-family:Inter,Helvetica,Arial,sans-serif;font-size:15px;line-height:1.55;color:#111\">{}</div>", kiqr_core::channels::markdown_to_html(text_md));
    let alternative = MultiPart::alternative_plain_html(text_md.to_string(), html);
    let msg = if files.is_empty() {
        builder.multipart(alternative)?
    } else {
        let mut mixed = MultiPart::mixed().multipart(alternative);
        for (name, bytes, mime) in files {
            let ct = ContentType::parse(&mime).unwrap_or(ContentType::parse("application/octet-stream").unwrap());
            let part: SinglePart = Attachment::new(name).body(bytes, ct);
            mixed = mixed.singlepart(part);
        }
        builder.multipart(mixed)?
    };
    let mut transport = match security.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host)?.port(port),
        // Plain: only for a relay on this machine or a private network.
        "none" | "plain" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&s.host).port(port),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host)?.port(port),
    };
    if let (Some(user), Some(pass)) = (&s.user, &s.password) {
        transport = transport.credentials(Credentials::new(user.clone(), pass.clone()));
    }
    transport.build().send(msg).await.with_context(|| format!("sending through {}:{port}", s.host))?;
    Ok(from.to_string())
}
