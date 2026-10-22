//! The web UI, built by Vite into `web/dist` and embedded at compile time.
//! Unknown paths fall back to `index.html` so client-side routing works.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist/"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, "no such API route").into_response();
    }
    let candidate = if path.is_empty() { "index.html" } else { path };
    let file = Assets::get(candidate).or_else(|| Assets::get("index.html"));
    match file {
        Some(f) => {
            let mime = mime_guess::from_path(if Assets::get(candidate).is_some() { candidate } else { "index.html" })
                .first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, if candidate.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" })
                .body(Body::from(f.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "web UI not built — run `npm run build` in web/").into_response(),
    }
}
