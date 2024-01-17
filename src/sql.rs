#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::AppState;
use anyhow::Result;
use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
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
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};

pub async fn get_sql_client() -> Client {
    // Connect to the database.
    let (client, connection) =
        tokio_postgres::connect("host=haus dbname=monitoring user=adam password=adam", NoTls)
            // tokio_postgres::connect("host=dev23 dbname=draculadb user=adam password=adam", NoTls)
            .await
            .unwrap();

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    client
}

#[debug_handler]
pub async fn stream_sql_response(
    State(state): State<AppState>,
    Query(params): Query<Vec<(String, String)>>,
) -> impl IntoResponse {
    let client = state.client;
    // Read the SQL contents from the file.
    let sql =
        fs::read_to_string("project/src/sql/samples.sql").expect("Unable to read the SQL file");
    let sql_files_content = read_sql_files().unwrap();
    println!("fff {:?}", sql_files_content);
    let sources: Vec<String> = params
        .iter()
        .filter(|(key, _)| key == "source")
        .map(|(_, value)| value)
        .cloned()
        .collect();

    // Execute the query and get results.
    let sql_params: Vec<String> = vec![];
    let it = client.query_raw(&sql, sql_params).await.unwrap();

    // Iterate over the stream to process each row.
    let json_stream: JsonStream = Box::pin(
        it.map_ok(|row| {
            let eventname = "stream1";
            let value: Json = row.get(0);
            // SSE
            format!("event: {}\ndata: {}\n\n", eventname, value)
        })
        .map_err(AxumError::new),
    );

    let (client2, connection2) =
        tokio_postgres::connect("host=haus dbname=monitoring user=adam password=adam", NoTls)
            .await
            .unwrap();

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection2.await {
            eprintln!("connection error: {}", e);
        }
    });
    let sql2: Vec<String> = vec![];
    let it2 = client2.query_raw(&sql, sql2).await.unwrap();
    let js2: JsonStream = Box::pin(
        it2.map_ok(|row| {
            let eventname = "stream2";
            let value: Json = row.get(0);
            // SSE
            format!("event: {}\ndata: {}\n\n", eventname, value)
        })
        .map_err(AxumError::new),
    );

    let body = Body::from_stream(select(json_stream, js2));

    // let body = Body::from_stream(json_stream);
    (
        // set status code
        StatusCode::NOT_FOUND,
        // headers with an array
        [("x-custom", "custom")],
        body,
    )
}

type JsonStream = Pin<Box<dyn Stream<Item = Result<String, AxumError>> + Send>>;
fn read_sql_files() -> Result<HashMap<String, String>> {
    let directory = Path::new("project/src/sql");
    assert!(directory.is_dir());
    Ok(fs::read_dir(directory)?
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|e| e.path().is_file().then(|| e.path()))
        })
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("sql"))
        .filter_map(|path| {
            let file_stem = path.file_stem()?.to_str()?.to_owned();
            let content = fs::read_to_string(&path).ok()?;
            Some((file_stem, content))
        })
        .collect())
}
