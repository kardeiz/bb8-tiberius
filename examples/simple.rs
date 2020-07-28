#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let conn_str = std::env::var("DB_CONN")?;

    let mgr = bb8_tiberius::ConnectionManager::build(conn_str.as_str())?;

    let pool = bb8::Pool::builder().max_size(2).build(mgr).await?;

    let mut conn = pool.get().await?;

    let res = conn.simple_query("SELECT @@version")
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
