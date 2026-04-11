use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct StaticAssets;

pub async fn static_handler(uri: Uri) -> Response<Body> {
    let path = uri.path();
    
    if path.starts_with("/ui") {
        let sub_path = path.strip_prefix("/ui").unwrap_or(path);
        let sub_path = sub_path.trim_start_matches('/');
        
        if sub_path.is_empty() || sub_path == "index.html" {
            return serve_index_html();
        }
        
        if let Some(content) = StaticAssets::get(sub_path) {
            let mime_type = mime_guess::from_path(sub_path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();
            
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type)
                .body(Body::from(content.data))
                .unwrap();
        }
        
        return serve_index_html();
    }
    
    let path = path.trim_start_matches('/');
    
    if path.is_empty() || path == "index.html" {
        return serve_index_html();
    }
    
    if let Some(content) = StaticAssets::get(path) {
        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .as_ref()
            .to_string();
        
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .body(Body::from(content.data))
            .unwrap()
    } else {
        serve_index_html()
    }
}

fn serve_index_html() -> Response<Body> {
    match StaticAssets::get("index.html") {
        Some(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(content.data))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("index.html not found"))
            .unwrap(),
    }
}

pub async fn index_handler() -> Response<Body> {
    serve_index_html()
}
