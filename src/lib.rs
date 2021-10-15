
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
    modify_tcp_stream:
        Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
    #[cfg(feature = "with-async-std")]
    modify_tcp_stream: Box<
        dyn Fn(&async_std::net::TcpStream) -> async_std::io::Result<()> + Send + Sync + 'static,
    >,
    #[cfg(feature = "sql-browser")]
    use_named_connection: bool,
}

impl ConnectionManager {
    /// Create a new `ConnectionManager`
    pub fn new(config: tiberius::Config) -> Self {
        Self { 
            config, 
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true)), 
            #[cfg(feature = "sql-browser")] 
            use_named_connection: false 
        }
    }

    /// Build a `ConnectionManager` from e.g. an ADO string
    pub fn build<I: IntoConfig>(config: I) -> Result<Self, Error> {
        Ok(config.into_config().map(Self::new)?)
    }

    #[cfg(feature = "sql-browser")]
    /// Use `tiberius::SqlBrowser::connect_named` to establish the TCP stream
    pub fn using_named_connection(mut self) -> Self {
        self.use_named_connection = true;
        self
    }
}

/// Runtime (`tokio` or `async-std` dependent parts)
#[cfg(feature = "with-tokio")]
pub mod rt {

    /// The connection type
    pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

    impl super::ConnectionManager {
        /// Perform some configuration on the TCP stream when generating connections
        pub fn with_modify_tcp_stream<F>(mut self, f: F) -> Self
        where
            F: Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static,
        {
            self.modify_tcp_stream = Box::new(f);
            self
        }

        #[cfg(feature = "sql-browser")]
        async fn connect_tcp(&self) -> Result<tokio::net::TcpStream, super::Error> {
            use tiberius::SqlBrowser;
            
            if self.use_named_connection {
                Ok(tokio::net::TcpStream::connect_named(&self.config).await?)
            } else {
                Ok(tokio::net::TcpStream::connect(self.config.get_addr()).await?)
            }
        }

        #[cfg(not(feature = "sql-browser"))]
        async fn connect_tcp(&self) -> std::io::Result<tokio::net::TcpStream> {
            tokio::net::TcpStream::connect(self.config.get_addr()).await
        }

        pub(crate) async fn connect_inner(&self) -> Result<Client, super::Error> {
            use tokio::net::TcpStream;
            use tokio_util::compat::TokioAsyncWriteCompatExt; //Tokio02AsyncWriteCompatExt;

            let tcp = self.connect_tcp().await?;

            (self.modify_tcp_stream)(&tcp)?;

            let client = match Client::connect(self.config.clone(), tcp.compat_write()).await {
                // Connection successful.
                Ok(client) => client,

                // The server wants us to redirect to a different address
                Err(tiberius::error::Error::Routing { host, port }) => {
                    let mut config = self.config.clone();

                    config.host(&host);
                    config.port(port);

                    let tcp = TcpStream::connect(config.get_addr()).await?;

                    (self.modify_tcp_stream)(&tcp)?;

                    // we should not have more than one redirect, so we'll short-circuit here.
                    tiberius::Client::connect(config, tcp.compat_write()).await?
                }

                // Other error happened
                Err(e) => Err(e)?,
            };

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
        pub fn with_modify_tcp_stream<F>(mut self, f: F) -> Self
        where
            F: Fn(&async_std::net::TcpStream) -> async_std::io::Result<()> + Send + Sync + 'static,
        {
            self.modify_tcp_stream = Box::new(f);
            self
        }

        #[cfg(feature = "sql-browser")]
        async fn connect_tcp(&self) -> tiberius::Result<async_std::net::TcpStream> {
            use tiberius::SqlBrowser;
            async_std::net::TcpStream::connect_named(&self.config).await
        }

        #[cfg(not(feature = "sql-browser"))]
        async fn connect_tcp(&self) -> std::io::Result<async_std::net::TcpStream> {
            async_std::net::TcpStream::connect(self.config.get_addr()).await
        }

        pub(crate) async fn connect_inner(&self) -> Result<Client, super::Error> {
            let tcp = self.connect_tcp().await?;

            (self.modify_tcp_stream)(&tcp)?;

            let client = match Client::connect(self.config.clone(), tcp).await {
                // Connection successful.
                Ok(client) => client,

                // The server wants us to redirect to a different address
                Err(tiberius::error::Error::Routing { host, port }) => {
                    let mut config = self.config.clone();

                    config.host(&host);
                    config.port(port);

                    let tcp = async_std::net::TcpStream::connect(config.get_addr()).await?;

                    (self.modify_tcp_stream)(&tcp)?;

                    // we should not have more than one redirect, so we'll short-circuit here.
                    tiberius::Client::connect(config, tcp).await?
                }

                // Other error happened
                Err(e) => Err(e)?,
            };

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

    async fn is_valid<'a, 'b, 'c>(&'a self, conn: &'b mut bb8::PooledConnection<'c,Self>) -> Result<(), Self::Error> {
        conn.simple_query("SELECT 1").await?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}
