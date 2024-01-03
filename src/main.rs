use tokio_postgres::{NoTls, Error, types::ToSql};
use serde_json::Value as Json;
use futures::{pin_mut, TryStreamExt};
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Connect to the database.
    let (client, connection) =
        tokio_postgres::connect("host=haus dbname=monitoring user=adam password=adam", NoTls).await?;

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
    let mut it = client.query_raw(&sql, params).await?;
    pin_mut!(it);

    // Iterate over the stream to process each row.
    while let Some(row) = it.try_next().await? {
        let value: Json = row.get(0);
        println!("{}", value);
    }

    Ok(())
}
