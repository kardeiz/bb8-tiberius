use bb8_tiberius::{Error, ConnectionManager, PoolExt};
use futures_state_stream::StateStream;
use futures::future::Future;

fn main() -> Result<(), Box<std::error::Error>> {

    let fut = {
        let conn_str = std::env::var("DB_CONN")?;

        futures::future::lazy(|| {

            let pool = bb8::Pool::builder()
                .max_size(2)
                .build_unchecked(ConnectionManager(conn_str));

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

    Ok(())
}

