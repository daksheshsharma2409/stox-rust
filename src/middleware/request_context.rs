use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use regex::Regex;
use std::sync::LazyLock;
use uuid::Uuid;

static REQUEST_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9._:-]{8,128}$").unwrap()
});

#[derive(Clone, Debug)]
pub struct RequestId(pub String);

pub async fn request_context_middleware(mut request: Request, next: Next) -> Response {
    let resolved_id = match request.headers().get("x-request-id") {
        Some(header_val) => {
            if let Ok(val_str) = header_val.to_str() {
                let trimmed = val_str.trim();
                if REQUEST_ID_REGEX.is_match(trimmed) {
                    trimmed.to_string()
                } else {
                    Uuid::new_v4().to_string()
                }
            } else {
                Uuid::new_v4().to_string()
            }
        }
        None => Uuid::new_v4().to_string(),
    };

    request.extensions_mut().insert(RequestId(resolved_id.clone()));

    let mut response = next.run(request).await;
    if let Ok(header_val) = resolved_id.parse() {
        response.headers_mut().insert("X-Request-Id", header_val);
    }

    response
}
