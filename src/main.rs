#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use anyhow::Result;
use axum::{
    body::Body,
    extract::Query,
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Error as AxumError, Router,
};
use axum_macros::debug_handler;
use futures::stream::select;
use futures::Stream;
use futures::{pin_mut, TryStreamExt};
use futures::{stream, StreamExt}; // Assumed 'futures' is in the dependencies
use serde_json::Value as Json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio_postgres::{types::ToSql, Client, Error, NoTls};
mod template;

use template::compile_template;
use template::get_page;

#[derive(Clone)]
struct AppState {
    client: Arc<Client>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // let res = compile_template(&PathBuf::from("templates/index.html"));
    let res = get_page("/weather".into(), PathBuf::from("templates"));
    match res {
        Ok(html) => println!("{}", html),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}

async fn serve() -> Result<(), Error> {
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
        .route("/stream", get(stream_sql_response))
        .with_state(state);

    // Run the application
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

type JsonStream = Pin<Box<dyn Stream<Item = Result<String, AxumError>> + Send>>;

#[debug_handler]
async fn stream_sql_response(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let client = state.client;
    // Read the SQL contents from the file.
    let sql = fs::read_to_string("sql/test.sql").expect("Unable to read the SQL file");
    println!("PA {:?}", params.get("q"));
    let sql_files_content = read_sql_files().unwrap();
    println!("fff {:?}", sql_files_content);

    let streams: Vec<&str> = params
        .get("q")
        .expect("No streams given")
        .split(',')
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
        .map_err(|e| AxumError::new(e)),
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
            println!("foo {}", value);
            // SSE
            format!("event: {}\ndata: {}\n\n", eventname, value)
        })
        .map_err(|e| AxumError::new(e)),
    );

    let body = Body::from_stream(select(json_stream, js2));

    // let body = Body::from_stream(json_stream);
    return (
        // set status code
        StatusCode::NOT_FOUND,
        // headers with an array
        [("x-custom", "custom")],
        body,
    );
}

fn read_sql_files() -> Result<HashMap<String, String>> {
    let directory = Path::new("sql");
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
