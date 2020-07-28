
/// The error container
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tiberius(#[from] tiberius::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Implemented for `&str` (ADO-style string) and `tiberius::Config`
pub trait IntoConfig {
    fn into_config(self) -> tiberius::Result<tiberius::Config>;
}

impl<'a> IntoConfig for &'a str {
    fn into_config(self) -> tiberius::Result<tiberius::Config> {
        tiberius::Config::from_ado_string(self)
    }
}

impl IntoConfig for tiberius::Config {
    fn into_config(self) -> tiberius::Result<tiberius::Config> {
        Ok(self)
    }
}

/// Implements `bb8::ManageConnection`
pub struct ConnectionManager {
    config: tiberius::Config,
    #[cfg(feature = "with-tokio")]
    modify_tcp_stream: Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
    #[cfg(feature = "with-async-std")]
    modify_tcp_stream: Box<dyn Fn(&async_std::net::TcpStream) -> async_std::io::Result<()> + Send + Sync + 'static>,
}

impl ConnectionManager {
    /// Create a new `ConnectionManager`
    pub fn new(config: tiberius::Config) -> Self {
        Self { 
            config, 
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true) )
        }
    }

    /// Build a `ConnectionManager` from e.g. an ADO string
    pub fn build<I: IntoConfig>(config: I) -> Result<Self, Error> {
        Ok(config.into_config().map(Self::new)?)
    }
}

/// Runtime (`tokio` or `async-std` dependent parts)
#[cfg(feature = "with-tokio")]
pub mod rt {

    /// The connection type
    pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

    impl super::ConnectionManager {

        /// Perform some configuration on the TCP stream when generating connections
        pub fn with_modify_tcp_stream<F>(mut self, f: F) -> Self where F: Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static {
            self.modify_tcp_stream = Box::new(f);
            self
        }

        pub(crate) async fn connect_inner(&self) -> Result<Client, super::Error> {
            use tokio::net::TcpStream;
            use tokio_util::compat::Tokio02AsyncWriteCompatExt;

            let tcp = TcpStream::connect(self.config.get_addr()).await?;

            (self.modify_tcp_stream)(&tcp)?;

            let client = tiberius::Client::connect(self.config.clone(), tcp.compat_write()).await?;

            Ok(client)
        }
    }    
}

#[cfg(feature = "with-async-std")]
pub mod rt {

    /// The connection type
    pub type Client = tiberius::Client<async_std::net::TcpStream>;

    impl super::ConnectionManager {

        /// Perform some configuration on the TCP stream when generating connections
        pub fn with_modify_tcp_stream<F>(mut self, f: F) -> Self where F: Fn(&async_std::net::TcpStream) -> async_std::io::Result<()> + Send + Sync + 'static {
            self.modify_tcp_stream = Box::new(f);
            self
        }

        pub(crate) async fn connect_inner(&self) -> Result<Client, super::Error> {

            let tcp = async_std::net::TcpStream::connect(self.config.get_addr()).await?;

            (self.modify_tcp_stream)(&tcp)?;

            let client = tiberius::Client::connect(self.config.clone(), tcp).await?;

            Ok(client)
        }
    }  
}

#[async_trait::async_trait]
impl bb8::ManageConnection for ConnectionManager {
    type Connection = rt::Client;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        Ok(self.connect_inner().await?)
    }

    async fn is_valid(&self, mut conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        conn.simple_query("SELECT 1").await?;
        Ok(conn)
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}