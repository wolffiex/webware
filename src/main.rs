#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::Result;

use axum::{
    body::Body,
    extract::Query,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Error as AxumError, Router,
};
use axum_macros::debug_handler;
use futures::stream::select;
use futures::Stream;
use futures::TryStreamExt;
use futures::{stream, StreamExt};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_postgres::{Client, NoTls};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
mod sql;
mod template;
use deadpool_postgres::Pool;
use tokio_stream::wrappers::UnboundedReceiverStream;

use sql::{create_pool, send_sql_result};
use template::get_page;

#[derive(Clone)]
pub struct AppState {
    client_pool: Arc<Pool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let now = Instant::now(); // get current time

    let res = get_page("/weather".into(), PathBuf::from("project/src/templates"))?;
    let elapsed = now.elapsed(); // get elapsed time
    println!("Compilation took: {:.2?}", elapsed);

    let client_pool = create_pool().await?;

    let state = AppState {
        client_pool: Arc::new(client_pool),
    };

    // Set up the router and routes
    let app = Router::new()
        .nest_service("/www", ServeDir::new("project/www"))
        .route("/api", get(stream_sql_response))
        .route_service("/index.js", ServeFile::new("www/index.js"))
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/www/images/favicon.ico") }),
        )
        .fallback(get(|| async { Ok::<Html<String>, Infallible>(Html(res)) }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Run the application
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[debug_handler]
async fn stream_sql_response(
    State(state): State<AppState>,
    Query(params): Query<Vec<(String, String)>>,
) -> impl IntoResponse {
    let sources: Vec<String> = params
        .iter()
        .filter_map(|(key, value)| match key.as_str() {
            "source" => Some(value),
            _ => None,
        })
        .cloned()
        .collect();

    let (tx, rx): (
        UnboundedSender<Result<String, anyhow::Error>>,
        UnboundedReceiver<Result<String, anyhow::Error>>,
    ) = unbounded_channel();

    tokio::spawn(async move {
        send_sql_result(state.client_pool, sources, tx)
            .await
            .unwrap();
    });

    let rx_stream = UnboundedReceiverStream::new(rx);
    let body = Body::from_stream(rx_stream);
    (
        StatusCode::OK,
        [
            ("Content-Type", "text/event-stream"),
            ("x-custom", "custom"),
        ],
        body,
    )
}
