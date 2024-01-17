#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::AppState;
use std::time::Instant;
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
use bytes::Bytes;
use deadpool_postgres::{Config, Manager, Pool, Runtime};
use futures::{stream::select, pin_mut};
use futures::Stream;
use futures::TryStreamExt;
use futures::{stream, StreamExt};
use serde_json::Value as Json;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_postgres::{Client, NoTls, RowStream};

pub async fn create_pool() -> Result<Pool> {
    let mut cfg = Config::default();
    cfg.dbname = Some("monitoring".to_string());
    cfg.user = Some("adam".to_string());
    cfg.password = Some("adam".to_string());
    cfg.host = Some("haus".to_string());
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
    Ok(pool)
}

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

pub async fn send_sql_result(
    client_pool: Arc<Pool>,
    sources: Vec<String>,
    tx: UnboundedSender<Result<String, anyhow::Error>>,
) -> Result<()> {
    // Read the SQL contents from the file.
    let sql =
        fs::read_to_string("project/src/sql/samples.sql").expect("Unable to read the SQL file");
    let sql_files_content = read_sql_files()?;

    // Iterate over the stream to process each row.
    // Execute the query and get results.
    let sql_params: Vec<String> = vec![];
    let now = Instant::now(); // get current time
    let client = client_pool.get().await.unwrap();
    let elapsed = now.elapsed(); // get elapsed time
    println!("Client took: {:.2?}", elapsed);
    let now = Instant::now(); // get current time
    let stream = client.query_raw(&sql, sql_params.iter()).await.unwrap();
    let elapsed = now.elapsed(); // get elapsed time
    println!("stream took: {:.2?}", elapsed);
    pin_mut!(stream);
    let now = Instant::now(); // get current time
    while let Some(Ok(row)) = stream.next().await {
        let eventname = "stream1";
        let maybe_value: Option<Json> = row.get(0);
        // tokio::time::sleep(Duration::from_secs(1)).await;
        if tx
            .send(match maybe_value {
                Some(value) => Ok(format!("event: {}\ndata: {}\n\n", eventname, value)),
                None => Err(anyhow::anyhow!("Missing value")),
            })
            .is_err()
        {
            eprintln!("Channel broke");
            break;
        }
    }
    let elapsed = now.elapsed(); // get elapsed time
    println!("finished map took: {:.2?}", elapsed);
    Ok(())
}

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
