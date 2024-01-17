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
use bytes::Bytes;
use futures::stream::select;
use futures::Stream;
use futures::TryStreamExt;
use futures::{stream, StreamExt};
use serde_json::Value as Json;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_postgres::{Client, NoTls};
use deadpool_postgres::{Runtime, Pool, Config, Manager};

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
    // println!("fff {:?}", sql_files_content);

    // Execute the query and get results.
    let sql_params: Vec<String> = vec![];
    let client = client_pool.get().await?;
    let it = client.query_raw(&sql, sql_params).await?;

    // Iterate over the stream to process each row.
    let _ = tokio::spawn(it.for_each(move |row_result| {
        let tx1a = tx.clone();
        async move {
            let event = row_result
                .map(|row| {
                    let eventname = "stream1";
                    let value: Json = row.get(0);
                    // SSE
                    format!("event: {}\ndata: {}\n\n", eventname, value)
                })
                .map_err(anyhow::Error::new);

            // tokio::time::sleep(Duration::from_secs(1)).await;
            if let Err(_) = tx1a.send(event) {
                // handle this error as you see fit
                println!("WHALKJER");
            }
        }
    }));
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
