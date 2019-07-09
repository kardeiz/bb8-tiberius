use bb8_tiberius::{Error, ConnectionManager};
use futures_state_stream::StateStream;
use futures::future::Future;

fn main() -> Result<(), Box<std::error::Error>> {

    let fut = {
        let conn_str = std::env::var("DB_CONN")?;

        futures::future::lazy(|| {

            let pool = bb8::Pool::builder()
                .max_size(2)
                .build_unchecked(ConnectionManager(conn_str));

            let rt = pool.clone().run(|pooled_conn| {
                pooled_conn.run(|conn| {
                    conn.simple_query("SELECT @@version")
                        .map_err(Error::from)
                        .map(|row| {
                            let val: &str = row.get(0);
                            String::from(val)
                        })
                        .collect()
                        .map(|(items, state)| {
                            println!("{:?}", &items);
                            ((), state)
                        })
                })
            });

            rt
        })
    };

    tokio::runtime::current_thread::block_on_all(fut).unwrap();

    Ok(())
}

