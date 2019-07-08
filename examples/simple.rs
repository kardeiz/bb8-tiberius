use bb8_tiberius::{Error, ConnectionManager};
use futures_state_stream::StateStream;
use futures::future::Future;

fn main() -> Result<(), Box<std::error::Error>> {
    let conn_str = std::env::var("DB_CONN")?;

    let fut = futures::future::lazy(|| {

        let pool = bb8::Pool::builder()
            .max_size(2)
            .build_unchecked(ConnectionManager(conn_str));

        let rt = pool.clone().run(|mut opt_conn| {
            let conn = opt_conn.unwrap();
            let rt = conn.simple_query("SELECT @@version")
                .map_err(|e| (Error::from(e), None))
                .and_then(|row| {
                    let val: &str = row.get(0);
                    Ok(String::from(val))
                })
                .collect()
                .map(|(items, state)| {
                    println!("{:?}", &items);
                    ((), None)
                });
            rt
        });

        rt
    });

    tokio::runtime::current_thread::block_on_all(fut).unwrap();

    Ok(())
}

