mod system;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app_state = system::AppState::new()?;

    let addr = std::env::var("HOST").unwrap_or("127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or("3000".to_string());
    let addr = format!("{}:{}", addr, port);
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let cors = tower_http::cors::CorsLayer::new()
        // Allow all origins
        .allow_origin(tower_http::cors::Any)
        // Allow common methods
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
            axum::http::Method::PATCH,
        ])
        // Allow common headers
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::CONTENT_TYPE,
        ])
        // Allow credentials (cookies, etc.)
        .allow_credentials(true);

    axum::serve(listener, app_state.router().layer(cors)).await?;

    Ok(())
}
