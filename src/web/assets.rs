//! Static assets embedded in the binary (no files to ship next to it).

use axum::extract::{Path, Query};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::ctx::ASSET_VERSION;

struct Asset {
    bytes: &'static [u8],
    content_type: &'static str,
}

fn lookup(path: &str) -> Option<Asset> {
    let asset = match path {
        "app.css" => Asset {
            bytes: include_bytes!("../../static/app.css"),
            content_type: "text/css; charset=utf-8",
        },
        "app.js" => Asset {
            bytes: include_bytes!("../../static/app.js"),
            content_type: "text/javascript; charset=utf-8",
        },
        "favicon.svg" => Asset {
            bytes: include_bytes!("../../static/favicon.svg"),
            content_type: "image/svg+xml",
        },
        "fonts/fraunces.woff2" => Asset {
            bytes: include_bytes!("../../static/fonts/fraunces.woff2"),
            content_type: "font/woff2",
        },
        _ => return None,
    };
    Some(asset)
}

#[derive(Debug, Default, Deserialize)]
pub struct VersionQuery {
    pub v: Option<String>,
}

pub async fn serve(Path(path): Path<String>, Query(q): Query<VersionQuery>) -> Response {
    let Some(asset) = lookup(&path) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    // Versioned URLs (the ones templates emit) are immutable; bare URLs revalidate often.
    let cache = if q.v.as_deref() == Some(ASSET_VERSION) {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=300"
    };
    (
        [
            (header::CONTENT_TYPE, asset.content_type.to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (header::ETAG, format!("\"{ASSET_VERSION}\"")),
        ],
        asset.bytes,
    )
        .into_response()
}
