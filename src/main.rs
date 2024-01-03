#![allow(dead_code)]
#![allow(unused_imports)]
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Error as AxumError, Router,
};
use axum_macros::debug_handler;
use futures::{pin_mut, TryStreamExt};
use serde_json::Value as Json;
use std::fs;
use std::sync::Arc;
use tokio_postgres::{types::ToSql, Client, Error, NoTls};

#[derive(Clone)]
struct AppState {
    client: Arc<Client>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Connect to the database.
    let (client, connection) =
        tokio_postgres::connect("host=haus dbname=monitoring user=adam password=adam", NoTls)
            .await
            .unwrap();

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let state = AppState {
        client: Arc::new(client),
    };

    // Set up the router and routes
    let app = Router::new()
        .route("/", get(stream_sql_response))
        .with_state(state);

    // Run the application
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[debug_handler]
async fn stream_sql_response(State(state): State<AppState>) -> impl IntoResponse {
    let client = state.client;
    // Read the SQL contents from the file.
    let sql = fs::read_to_string("sql/test.sql").expect("Unable to read the SQL file");

    // Execute the query and get results.
    let params: Vec<String> = vec![];
    let it = client.query_raw(&sql, params).await.unwrap();

    // Iterate over the stream to process each row.
    let json_stream = Box::pin(it.map_ok(|row| {
        let eventname = "stream1";
        let value: Json = row.get(0);
        // Convert the JSON value to a string plus a newline to separate the values
        let json_string = format!("event: {}\ndata: {}\n\n", eventname, value);
        json_string
    }))
    .map_err(|e| AxumError::new(e));

    let body = Body::from_stream(json_stream);
    return (
        // set status code
        StatusCode::NOT_FOUND,
        // headers with an array
        [("x-custom", "custom")],
        body,
    );
}
