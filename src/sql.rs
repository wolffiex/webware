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
use deadpool_postgres::{Client, Config, Manager, Pool, Runtime};
use futures::future;
use futures::Stream;
use futures::TryStreamExt;
use futures::{pin_mut, stream::select};
use futures::{stream, StreamExt};
use serde_json::Value as Json;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_postgres::{NoTls, RowStream};

pub async fn create_pool() -> Result<Pool> {
    let mut cfg = Config::default();
    cfg.dbname = Some("monitoring".to_string());
    cfg.user = Some("adam".to_string());
    cfg.password = Some("adam".to_string());
    cfg.host = Some("haus".to_string());
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
    Ok(pool)
}

type ResultSender = UnboundedSender<Result<String, anyhow::Error>>;
pub async fn send_sql_results(
    client_pool: Arc<Pool>,
    sources: Vec<String>,
    tx: ResultSender,
) -> Result<()> {
    println!("sour: {:?}", sources);
    futures::stream::iter(sources.into_iter())
        .for_each_concurrent(None, |source| {
            let tx_clone = tx.clone();
            let pool_clone = client_pool.clone();
            async move {
                let client = pool_clone.get().await.unwrap();
                let sql_params: Vec<String> = vec![];
                let now = Instant::now(); // get current time
                let elapsed = now.elapsed(); // get elapsed time
                println!("Client took: {:.2?}", elapsed);
                let now = Instant::now(); // get current time
                let sql = "SELECT * FROM weather";
                let stream = client.query_raw(sql, sql_params.iter()).await.unwrap();
                let elapsed = now.elapsed(); // get elapsed time
                println!("stream took: {:.2?}", elapsed);
                pin_mut!(stream);
                let now = Instant::now(); // get current time
                while let Some(Ok(row)) = stream.next().await {
                    let eventname = "stream1";
                    let maybe_value: Option<Json> = row.get(0);
                    // tokio::time::sleep(Duration::from_secs(1)).await;
                    if tx_clone
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
            }
        })
        .await;
    Ok(())
}
