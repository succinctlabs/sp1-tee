use std::collections::HashMap;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::{get, post, Router},
};

use tokio::net::TcpListener;

lazy_static::lazy_static! {
    static ref REDIRECTS: HashMap<String, String> = {
        let mut map = HashMap::new();
        map.insert("4.0.0-rc.3".to_string(), "todo".to_string());

        map
    };
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        // Health check route
        .route("/", get(|| async { StatusCode::OK.into_response() }))
        // Handle the redirect
        .route("/execute", post(handle_redirect));

    let listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Failed to serve");
}

async fn handle_redirect(req: axum::extract::Request) -> impl IntoResponse {
    match req.headers().get("X-SP1-Version") {
        Some(version) => {
            let destination = REDIRECTS.get(version.to_str().expect("Invalid version header"));
            match destination {
                Some(destination) => return Redirect::to(destination).into_response(),
                None => return (StatusCode::NOT_FOUND, "Version not found").into_response(),
            }
        }
        None => return (StatusCode::BAD_REQUEST, "Version header not found").into_response(),
    }
}
