use std::sync::Arc;

use anyhow::Context;
use axum::{extract::State, response::IntoResponse};
use tokio::sync::RwLock;

mod types;

#[derive(Debug, Clone)]
pub struct AppState {
    messages: Arc<RwLock<(Vec<Message>, time::PrimitiveDateTime)>>,
    config: Config,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub queue_size: usize,
    pub max_message_size: usize,
    pub max_author_size: usize,
    pub max_age: time::Duration,
    pub key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub message: String,
    pub author: String,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config {
            queue_size: std::env::var("QUEUE_SIZE")
                .unwrap_or("100".to_string())
                .parse()?,
            max_message_size: std::env::var("MAX_MESSAGE_SIZE")
                .unwrap_or("1024".to_string())
                .parse()?,
            max_author_size: std::env::var("MAX_AUTHOR_SIZE")
                .unwrap_or("50".to_string())
                .parse()?,
            max_age: time::Duration::minutes(
                std::env::var("MAX_AGE")
                    .unwrap_or("5".to_string())
                    .parse()?,
            ),
            key: std::env::var("API_KEY").context("`API_KEY` not present")?,
        };
        Ok(Self {
            messages: Arc::new(RwLock::new((Vec::new(), get_now()))),
            config,
        })
    }

    pub fn router(self) -> axum::Router<()> {
        axum::Router::new()
            .route("/message", axum::routing::post(add_message))
            .route("/message", axum::routing::get(get_messages))
            .layer(axum::middleware::from_fn_with_state(
                self.config.clone(),
                async |State(state): State<Config>,
                       req: axum::extract::Request,
                       next: axum::middleware::Next| {
                    let header = req
                        .headers()
                        .get("x-api-key")
                        .and_then(|inner| inner.to_str().ok());
                    match header {
                        Some(value) if value == state.key => next.run(req).await,
                        _ => axum::http::StatusCode::UNAUTHORIZED.into_response(),
                    }
                },
            ))
            .route("/health", axum::routing::get(health))
            .with_state(self)
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn add_message(
    State(state): State<AppState>,
    axum::Json(message): axum::Json<Message>,
) -> axum::http::StatusCode {
    let mut messages = state.messages.write().await;

    let output = match insert_message(&state.config, &mut messages, message) {
        Ok(()) => axum::http::StatusCode::CREATED,
        Err(status) => status,
    };

    if output.is_success() {
        tracing::debug!(count = messages.0.len(), "added message");
    }

    output
}

async fn get_messages(State(state): State<AppState>) -> axum::Json<Vec<Message>> {
    let messages = state.messages.read().await;

    tracing::debug!(count = messages.0.len(), "returning messages");

    axum::Json(messages.0.clone())
}

fn insert_message(
    config: &Config,
    (queue, last): &mut (Vec<Message>, time::PrimitiveDateTime),
    message: Message,
) -> Result<(), axum::http::StatusCode> {
    if get_now() - *last > config.max_age {
        queue.clear();
        *last = get_now();
    }

    if message.message.len() > config.max_message_size {
        tracing::error!(author = %message.author, length = message.message.len(), "message too large");
        return Err(axum::http::StatusCode::PAYLOAD_TOO_LARGE);
    }

    if queue.iter().filter(|m| m.author == message.author).count() >= config.max_author_size {
        tracing::error!(author = %message.author, "too many messages");
        return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    queue.push(message);

    if queue.len() > config.queue_size {
        queue.remove(0);
    }

    Ok(())
}

fn get_now() -> time::PrimitiveDateTime {
    let now = time::OffsetDateTime::now_utc();
    time::PrimitiveDateTime::new(now.date(), now.time())
}
