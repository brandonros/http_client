use std::net::ToSocketAddrs;

use alloc::string::{String, ToString};
use async_io::Async;
use async_tls::TlsConnector;
use http::Request;
use simple_error::{box_err, SimpleResult};

use crate::platform::AsyncConnection;

pub struct AsyncConnectionFactory;

impl AsyncConnectionFactory {
    // Extracts the scheme, host, and port from the request URI
    fn extract_host_from_request<T>(req: &Request<T>) -> SimpleResult<(String, String, u16)> {
        let uri = req.uri();
        let authority = uri.authority().ok_or("No authority found in URI")?;
        let scheme = uri.scheme_str().ok_or("No scheme found in URI")?;

        let host = authority.host();
        let port = authority.port_u16().unwrap_or_else(|| match scheme {
            "http" => 80,
            "https" => 443,
            "ws" => 80,
            "wss" => 443,
            _ => return 0,
        });

        if port == 0 {
            return Err(box_err!("Unsupported URL scheme"));
        }

        Ok((scheme.to_string(), host.to_string(), port))
    }

    pub async fn connect<T: core::fmt::Debug>(request: &Request<T>) -> SimpleResult<Box<dyn AsyncConnection>> {
        log::debug!("request = {request:?}");

        // Extract the scheme, host, and port from the request
        let (scheme, host, port) = Self::extract_host_from_request(request)?;
        let addr = alloc::format!("{host}:{port}")
            .to_socket_addrs()?
            .next()
            .ok_or("Failed to resolve host")?;
        let stream = Async::<std::net::TcpStream>::connect(addr).await?;

        // Optionally add TLS based on the scheme
        let stream: Box<dyn AsyncConnection> = if scheme == "https" || scheme == "wss" {
            let tls_connector = TlsConnector::new();
            Box::new(tls_connector.connect(&host, stream).await?)
        } else {
            Box::new(stream)
        };

        Ok(stream)
    }
}
