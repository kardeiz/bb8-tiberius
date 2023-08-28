# bb8-tiberius

Use [bb8](https://crates.io/crates/bb8) (pool manager for async connections) with [Tiberius](https://crates.io/crates/tiberius) (MSSQL driver for Rust).

## Usage

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn_str = std::env::var("DB_CONN")?;

    let mgr = bb8_tiberius::ConnectionManager::build(conn_str.as_str())?;

    let pool = bb8::Pool::builder().max_size(2).build(mgr).await?;

    let mut conn = pool.get().await?;

    let res = conn
        .simple_query("SELECT @@version")
        .await?
        .into_first_result()
        .await?
        .into_iter()
        .map(|row| {
            let val: &str = row.get(0).unwrap();
            String::from(val)
        })
        .collect::<Vec<_>>();

    println!("{:?}", &res);

    Ok(())
}
```

If using a [named instance](https://learn.microsoft.com/en-us/sql/database-engine/configure-windows/logging-in-to-sql-server?view=sql-server-ver16#format-for-specifying-the-name-of-sql-server) to connect, use the `using_named_connection` function:

```rust
let mgr = bb8_tiberius::ConnectionManager::build(conn_str.as_str())?.using_named_connection();
```
