use futures::future::{self, Future, IntoFuture, Either};
use futures_state_stream::StateStream;

#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum Error {
    #[display(fmt = "Tiberius: {:?}", _0)]
    Tiberius(tiberius::Error),
    #[display(fmt = "Connection removed")]
    EmptyConnection
}

#[derive(Debug, Clone)]
pub struct ConnectionManager(pub String);

pub type SqlConnection = tiberius::SqlConnection<Box<tiberius::BoxableIo>>;

impl bb8::ManageConnection for ConnectionManager {
    /// The connection type this manager deals with.
    type Connection = PooledConnection;
    /// The error type returned by `Connection`s.
    type Error = Error;

    /// Attempts to create a new connection.
    fn connect(&self) -> Box<Future<Item = Self::Connection, Error = Self::Error> + Send> {
        Box::new(tiberius::SqlConnection::connect(&self.0).map(Some).map(PooledConnection).from_err())
    }
    /// Determines if the connection is still connected to the database.
    fn is_valid(
        &self,
        conn: Self::Connection,
    ) -> Box<Future<Item = Self::Connection, Error = (Self::Error, Self::Connection)> + Send> {
        match conn.0 {
            Some(conn) => {
                let rt = conn
                    .simple_query("SELECT 1").for_each(|_| Ok(()))
                    .from_err()
                    .then(move |r| match r {
                        Ok(conn) => Ok(PooledConnection(Some(conn))),
                        Err(e) => Err((e, PooledConnection::default())),
                    });
                Box::new(rt)
            },
            None => Box::new(future::err((Error::EmptyConnection, PooledConnection::default())))
        }
    }
    /// Synchronously determine if the connection is no longer usable, if possible.
    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.0.is_none()
    }
}

#[derive(Default, derive_more::From)]
pub struct PooledConnection(pub Option<SqlConnection>);

impl PooledConnection {
    pub fn run<'a, T, E, U, F>(
        self,
        f: F,
    ) -> impl Future<Item = (T, Self), Error = (E, Self)> + Send + 'a
    where
        F: FnOnce(SqlConnection) -> U + Send + 'a,
        U: IntoFuture<Item = (T, SqlConnection), Error = E> + Send + 'a,
        U::Future: Send + 'a,
        E: From<Error> + Send + 'a,
        T: Send + 'a,
    {
        match self.0 {
            Some(conn) => Either::A(f(conn)
                .into_future()
                .map(|(t, conn)| (t, PooledConnection(Some(conn))))
                .map_err(|e| (e.into(), PooledConnection::default()))),
            None => 
                Either::B(future::err((Error::EmptyConnection.into(), PooledConnection::default())))
        }
    }
}
