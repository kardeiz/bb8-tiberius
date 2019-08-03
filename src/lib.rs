use futures::future::{self, Either, Future, IntoFuture};
use futures_state_stream::StateStream;

#[derive(Debug)]
pub enum Error {
    Tiberius(tiberius::Error),
    EmptyConnection,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Tiberius(ref e) => write!(f, "{:?}", e),
            Error::EmptyConnection => write!(f, "Connection removed"),
        }
    }
}

impl From<tiberius::Error> for Error {
    fn from(t: tiberius::Error) -> Self {
        Error::Tiberius(t)
    }
}

impl std::error::Error for Error {}

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
        println!("{:?}", "GETTING NEW CONN");
        Box::new(
            tiberius::SqlConnection::connect(&self.0).map(Some).map(PooledConnection).from_err(),
        )
    }
    /// Determines if the connection is still connected to the database.
    fn is_valid(
        &self,
        conn: Self::Connection,
    ) -> Box<Future<Item = Self::Connection, Error = (Self::Error, Self::Connection)> + Send> {
        match conn.0 {
            Some(conn) => {
                let rt =
                    conn.simple_query("SELECT 1").for_each(|_| Ok(())).from_err().then(move |r| {
                        match r {
                            Ok(conn) => Ok(PooledConnection(Some(conn))),
                            Err(e) => Err((e, PooledConnection(None))),
                        }
                    });
                Box::new(rt)
            }
            None => Box::new(future::err((Error::EmptyConnection, PooledConnection(None)))),
        }
    }
    /// Synchronously determine if the connection is no longer usable, if possible.
    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.0.is_none()
    }
}

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
            Some(conn) => Either::A(
                f(conn)
                    .into_future()
                    .map(|(t, conn)| (t, PooledConnection(Some(conn))))
                    .map_err(|e| (e.into(), PooledConnection(None))),
            ),
            None => Either::B(future::err((Error::EmptyConnection.into(), PooledConnection(None)))),
        }
    }
}

pub trait PoolExt {
    fn run_wrapped<'a, T, E, U, F>(
        &self,
        f: F,
    ) -> Box<Future<Item = T, Error = bb8::RunError<E>> + Send + 'a>
    where
        F: FnOnce(SqlConnection) -> U + Send + 'a,
        U: IntoFuture<Item = (T, SqlConnection), Error = E> + Send + 'a,
        U::Future: Send + 'a,
        E: From<Error> + Send + 'a,
        T: Send + 'a;
}

impl PoolExt for bb8::Pool<ConnectionManager> {
    fn run_wrapped<'a, T, E, U, F>(
        &self,
        f: F,
    ) -> Box<Future<Item = T, Error = bb8::RunError<E>> + Send + 'a>
    where
        F: FnOnce(SqlConnection) -> U + Send + 'a,
        U: IntoFuture<Item = (T, SqlConnection), Error = E> + Send + 'a,
        U::Future: Send + 'a,
        E: From<Error> + Send + 'a,
        T: Send + 'a,
    {
        Box::new(self.run(|pooled_conn| pooled_conn.run(f)))
    }
}
