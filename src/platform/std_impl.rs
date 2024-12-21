use alloc::{boxed::Box, vec::Vec};
use async_io::Async;
use async_tls::client::TlsStream;
use futures_lite::{AsyncRead, AsyncWrite, io::BufReader, AsyncWriteExt};
use http::{Request, Response};
use simple_error::SimpleResult;
use std::net::TcpStream;

use crate::{request, response};

// Move AsyncConnection trait and impls here
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

pub struct StdConnection {
    stream: Box<dyn AsyncConnection>,
}

impl StdConnection {
    pub async fn new(stream: Box<dyn AsyncConnection>) -> Self {
        Self { stream }
    }

    pub async fn send_request<T>(
        &mut self,
        request: &Request<T>
    ) -> SimpleResult<Response<Vec<u8>>> 
    where 
        T: AsRef<[u8]>
    {
        // Write the HTTP request to the stream
        let serialized_request = request::serialize_http_request(request)?;
        log::debug!("serialized_request = {serialized_request}");
        self.stream.write_all(serialized_request.as_bytes()).await?;
        self.stream.flush().await?;

        // Write request body if there is one
        if let Some(body) = request.body().as_ref() {
            if !body.as_ref().is_empty() {
                self.stream.write_all(body.as_ref()).await?;
                self.stream.flush().await?;
            }
        }

        // Read and parse the response
        let mut reader = BufReader::new(&mut self.stream);
        let response_status_line = response::read_response_status_line(&mut reader).await?;
        log::debug!("response_status_line = {response_status_line}");
        let (response_version, response_status) = response::parse_response_status_line(&response_status_line)?;
        let response_headers = response::read_response_headers(&mut reader).await?;
        log::debug!("response_headers = {response_headers:?}");
        let response_body = response::read_response_body(&mut reader, &response_headers).await?;
        log::debug!("response_body = {response_body:02x?}");

        // Build response
        let mut response = Response::builder()
            .status(response_status)
            .version(response_version)
            .body(response_body)?;
        *response.headers_mut() = response_headers;

        Ok(response)
    }
}
