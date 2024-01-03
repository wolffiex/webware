#![allow(dead_code)]
#![allow(unused_imports)]
use tokio_postgres::{NoTls, Error, types::ToSql};
use serde_json::Value as Json;
use futures::{pin_mut, TryStreamExt};
use std::fs;
use axum::{
    response::{Response, IntoResponse},
    http::{StatusCode, HeaderMap, Uri, header},
    routing::get,
    Error as AxumError,
    Router, 
    body::Body,
};
use axum_macros::debug_handler;


#[tokio::main]
async fn main() -> Result<(), Error> {
    // Set up the router and routes
    let app = Router::new()
        .route("/", get(stream_sql_response));

    // Run the application
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();


    Ok(())
}

#[debug_handler]
async fn stream_sql_response() -> impl IntoResponse {
    // Connect to the database.
    let (client, connection) =
        tokio_postgres::connect("host=haus dbname=monitoring user=adam password=adam", NoTls).await.unwrap();

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // Read the SQL contents from the file.
    let sql = fs::read_to_string("sql/test.sql").expect("Unable to read the SQL file");

    // Execute the query and get results.
    // Assuming the SQL script contains a query that returns rows.
    // let params: [&dyn ToSql] = [];
    let params: Vec<String> = vec![];
    let it = client.query_raw(&sql, params).await.unwrap();

    // Iterate over the stream to process each row.
    let json_stream = Box::pin(it.map_ok(|row| {
        let value: Json = row.get(0);
        // Convert the JSON value to a string plus a newline to separate the values
        let json_string = value.to_string() + "\n";
        json_string
    })).map_err(|e| AxumError::new(e));

    let body = Body::from_stream(json_stream);
    return (
        // set status code
        StatusCode::NOT_FOUND,
        // headers with an array
        [("x-custom", "custom")],
        body,
    )

}
