use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::Response;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../admin-ui/build"]
#[prefix = ""]
struct AdminAssets;

pub async fn admin_ui_handler(request: Request) -> Response {
    let path = request
        .uri()
        .path()
        .strip_prefix("/admin/")
        .or_else(|| request.uri().path().strip_prefix("/admin"))
        .unwrap_or("")
        .trim_start_matches('/');

    let path = if path.is_empty() { "index.html" } else { path };

    let file = AdminAssets::get(path).or_else(|| AdminAssets::get("index.html"));

    match file {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}
