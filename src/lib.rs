use futures::future::{self, Future};
use futures_state_stream::StateStream;

#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum Error {
    #[display(fmt = "Tiberius error: {:?}", _0)]
    Tiberius(tiberius::Error),
    #[display(fmt = "Connection removed")]
    EmptyConnection
}

#[derive(Debug, Clone)]
pub struct ConnectionManager(pub String);

impl bb8::ManageConnection for ConnectionManager {
    /// The connection type this manager deals with.
    type Connection = Option<tiberius::SqlConnection<Box<tiberius::BoxableIo>>>;
    /// The error type returned by `Connection`s.
    type Error = Error;

    /// Attempts to create a new connection.
    fn connect(&self) -> Box<Future<Item = Self::Connection, Error = Self::Error> + Send> {
        Box::new(tiberius::SqlConnection::connect(&self.0).map(Some).from_err())
    }
    /// Determines if the connection is still connected to the database.
    fn is_valid(
        &self,
        conn: Self::Connection,
    ) -> Box<Future<Item = Self::Connection, Error = (Self::Error, Self::Connection)> + Send> {
        match conn {
            Some(conn) => {
                let rt = conn
                    .simple_query("SELECT 1").for_each(|_| Ok(()))
                    .from_err()
                    .then(move |r| match r {
                        Ok(conn) => Ok(Some(conn)),
                        Err(e) => Err((e, None)),
                    });
                Box::new(rt)
            },
            None => Box::new(future::err((Error::EmptyConnection, None)))
        }
    }
    /// Synchronously determine if the connection is no longer usable, if possible.
    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.is_none()
    }
}