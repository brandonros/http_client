use std::net::TcpStream;

use async_io::Async;
use async_tls::client::TlsStream;
use futures_lite::{AsyncRead, AsyncWrite};

pub trait AsyncConnection: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    fn is_encrypted(&self) -> bool;
}

impl AsyncConnection for Async<TcpStream> {
    fn is_encrypted(&self) -> bool {
        false
    }
}

impl AsyncConnection for TlsStream<Async<TcpStream>> {
    fn is_encrypted(&self) -> bool {
        true
    }
}
