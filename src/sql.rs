#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::{AppState, cache::compute_cache_key};
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
use rayon::prelude::*;
use serde_json::Value as Json;
use std::convert::Infallible;
use std::fs;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_postgres::{NoTls, RowStream, Statement};

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
    statements: &StatementCollection,
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

pub struct StatementCollection {
    directory: PathBuf,
    cache_key: u64,
    cache: HashMap<String, Vec<Statement>>,
}

impl StatementCollection {
    pub fn new(directory: PathBuf) -> Self {
        StatementCollection {
            directory,
            cache_key: 0,
            cache: HashMap::new(),
        }
    }

    pub fn check(&self) -> bool {
        let new_key = compute_cache_key(&self.directory).unwrap();
        self.cache_key != new_key
    }

    pub async fn recompile(&mut self, client_pool: Arc<Pool>) -> Result<()> {
        let new_key = compute_cache_key(&self.directory).unwrap();
        if self.cache_key != new_key {
            self.cache_key = new_key;
            self.prepare_statements(client_pool).await?;
        }
        Ok(())
    }

    pub async fn prepare_statements(&mut self, client_pool: Arc<Pool>) -> Result<()> {
        let now = Instant::now(); // get current time
        let entries: Vec<_> = fs::read_dir(self.directory.clone())?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        let file_queries = entries
            .into_par_iter()
            .map(|path_buf| {
                let file = File::open(path_buf.clone())?;
                let mut reader = BufReader::new(file);
                let mut file_content = String::new();
                reader.read_to_string(&mut file_content)?;
                let queries: Vec<String> = file_content
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let fname = path_buf
                    .strip_prefix(&self.directory)?
                    .to_path_buf()
                    .into_os_string()
                    .to_string_lossy()
                    .to_string();
                Ok((fname, queries))
            })
            .collect::<Result<Vec<(String, Vec<String>)>>>()?;

        let mut tasks = Vec::new();

        // Loop over file_queries and spawn tasks
        for (file_name, queries) in file_queries {
            let client = client_pool.get().await?; // Clone the client
            let task = tokio::spawn(async move {
                let rn = Instant::now();
                let mut statements = Vec::new();
                for query in queries {
                    println!("q {:?}", query);
                    statements.push(client.prepare(query.as_str()).await.unwrap());
                }
                let ele = now.elapsed(); // get elapsed time
                println!("one preparation took {:?}", ele);
                (file_name, statements)
            });
            tasks.push(task);
        }

        // Await all tasks to completion and insert results into the cache
        for task in tasks {
            let (file_name, statements) = task.await?;
            self.cache.insert(file_name, statements);
        }

        let elapsed = now.elapsed(); // get elapsed time
        println!("Statement preparation took {:?}", elapsed);
        Ok(())
    }
}
