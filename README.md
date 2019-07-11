# bb8-tiberius

Use [bb8](https://crates.io/crates/bb8) (pool manager for async connections) with [Tiberius](https://crates.io/crates/tiberius) (MSSQL driver for Rust).

## Usage

```rust
use bb8_tiberius::{ConnectionManager, Error, PoolExt};
use futures::future::Future;
use futures_state_stream::StateStream;

let fut = {
    let conn_str = std::env::var("DB_CONN")?;

    futures::future::lazy(|| {
        let pool =
            bb8::Pool::builder().max_size(10).build_unchecked(ConnectionManager(conn_str));

        let rt = pool.run_wrapped(|conn| {
            conn.simple_query("SELECT @@version")
                .map_err(Error::from)
                .map(|row| {
                    let val: &str = row.get(0);
                    String::from(val)
                })
                .collect()
        });

        rt.map(|x| println!("{}", x.join(", "))).map_err(|_| ())
    })
};

tokio::run(fut);
```
