#[cfg(not(any(feature = "futures", feature = "futures-lite")))]
compile_error!("You must enable either the `futures` or `futures-lite` feature to build this crate.");

use std::net::ToSocketAddrs;
use std::str::FromStr;

use async_io::Async;
use async_tls::TlsConnector;
use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, Version};

#[cfg(feature = "futures")]
mod futures_imports {
    pub use futures::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
}

#[cfg(feature = "futures-lite")]
mod futures_lite_imports {
    pub use futures_lite::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
}

#[cfg(feature = "futures")]
use futures_imports::*;

#[cfg(feature = "futures-lite")]
use futures_lite_imports::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

trait AsyncConn: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin> AsyncConn for T {}

fn extract_host_from_request<Req>(req: &Request<Req>) -> Result<(String, String, u16)> {
    let uri = req.uri();
    let authority = uri.authority().ok_or("No authority found in URI")?;
    let scheme = uri.scheme_str().ok_or("No scheme found in URI")?;

    // Extract host and optional port from the authority
    let host = authority.host();
    let port = authority.port_u16().unwrap_or_else(|| match scheme {
        "http" => 80,   // Default port for HTTP
        "https" => 443, // Default port for HTTPS
        _ => 0,  // Indicate unknown scheme
    });

    if port == 0 {
        return Err("Unsupported URL scheme, only HTTP and HTTPS are supported".into());
    }

    Ok((scheme.to_string(), host.to_string(), port))
}

fn serialize_http_request<Req>(req: &Request<Req>) -> Result<String> {
    let method = req.method();
    let uri = req.uri();

    // Extract only the path and query from the URI
    let path_and_query = uri.path_and_query().map_or("/", |pq| pq.as_str());

    let version = match req.version() {
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1", // Default to HTTP/1.1 if uncertain
    };

    let mut request_line = format!("{method} {path_and_query} {version}\r\n");

    // Add headers
    for (name, value) in req.headers() {
        request_line.push_str(&format!("{}: {}\r\n", name.as_str(), value.to_str()?));
    }

    // Add an extra line to indicate the end of the header section
    request_line.push_str("\r\n");

    Ok(request_line)
}

async fn read_response_headers<S>(reader: &mut BufReader<S>) -> Result<HeaderMap<HeaderValue>>
where
    S: AsyncRead + Unpin,
{
    let mut headers = HeaderMap::new();
    let mut line = String::new();

    // Read lines until an empty line is reached (headers end)
    while reader.read_line(&mut line).await? != 0 && line != "\r\n" {
        // Parse the header line into a key-value pair and insert it into the map
        if let Some((key, value)) = line.split_once(": ") {
            let key = key.to_lowercase();
            let value = value.trim_end_matches(|c: char| c == '\r' || c == '\n');
            let header_name = HeaderName::from_str(&key)?;
            let header_value = HeaderValue::from_str(value)?;
            headers.insert(header_name, header_value);
        } else {
            log::warn!("Failed to parse header line: {line}");
        }
        line.clear(); // Clear the buffer for the next line
    }

    Ok(headers)
}

async fn read_chunked_body<S>(reader: &mut BufReader<S>) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut body = Vec::new();
    let mut chunk_size_line = String::new();

    loop {
        // Read the chunk size line
        reader.read_line(&mut chunk_size_line).await?;
        let chunk_size = usize::from_str_radix(chunk_size_line.trim(), 16)?;

        // Break on a zero-size chunk (end of the body)
        if chunk_size == 0 {
            break;
        }

        // Read the chunk data
        let mut chunk = vec![0; chunk_size];
        reader.read_exact(&mut chunk).await?;
        body.extend_from_slice(&chunk);

        // Read the trailing CRLF after the chunk
        let mut crlf = [0; 2];
        reader.read_exact(&mut crlf).await?;
        if &crlf != b"\r\n" {
            return Err("Invalid chunked encoding: missing CRLF".into());
        }
        chunk_size_line.clear();
    }

    Ok(body)
}

pub async fn send_http_request<Req, Res>(request: &Request<Req>) -> Result<Response<Res>>
where
    Req: std::fmt::Debug + PartialEq<()>,
    Res: std::fmt::Debug + Sized + std::convert::From<String>,
{
    log::debug!("request = {request:?}");

    // Open a TCP socket to the host
    let (scheme, host, port) = extract_host_from_request(request)?;
    let addr = format!("{host}:{port}").to_socket_addrs()?.next().ok_or("Failed to resolve host")?;
    let stream = Async::<std::net::TcpStream>::connect(addr).await?;

    // Optionally add TLS based on the scheme
    let mut stream: Box<dyn AsyncConn> = if scheme == "https" {
        let tls_connector = TlsConnector::new();
        Box::new(tls_connector.connect(&host, stream).await?)
    } else {
        Box::new(stream)
    };

    // Write the HTTP request to the stream
    let serialized_request = serialize_http_request(request)?;
    stream.write_all(serialized_request.as_bytes()).await?;
    stream.flush().await?;

    // Write request body if there is one
    let request_body = request.body();
    if *request_body != () {
        todo!() // Handle non-empty request body
    }

    // Read the response status line
    let mut reader = BufReader::new(stream);
    let mut response_status_line = String::new();
    reader.read_line(&mut response_status_line).await?;

    // Parse the response status line
    let response_status_line_parts: Vec<&str> = response_status_line.split_whitespace().collect();
    if response_status_line_parts.len() < 2 {
        return Err("Failed to parse response status line".into());
    }
    let response_version = match response_status_line_parts[0] {
        "HTTP/1.0" => Version::HTTP_10,
        "HTTP/1.1" => Version::HTTP_11,
        "HTTP/2.0" => Version::HTTP_2,
        _ => return Err("Unsupported HTTP version".into()),
    };
    let response_status = StatusCode::from_u16(response_status_line_parts[1].parse()?)?;

    // Read the response headers
    let response_headers = read_response_headers(&mut reader).await?;
    log::debug!("response_headers = {response_headers:?}");

    // Read the response body
    let response_body = if let Some(content_length_value) = response_headers.get("content-length") {
        // Read the response body based on the Content-Length
        let content_length = content_length_value.to_str()?.parse::<usize>()?;
        let mut response_body = vec![0u8; content_length];
        reader.read_exact(&mut response_body).await?;
        response_body
    } else if let Some(transfer_encoding) = response_headers.get("transfer-encoding") {
        if transfer_encoding == "chunked" {
            read_chunked_body(&mut reader).await?
        } else {
            todo!() // Handle other transfer encodings if needed
        }
    } else {
        // Read until end on HTTP/1.0 connection close
        let mut response_body = Vec::with_capacity(8 * 1024 * 1024);
        let num_bytes_read = reader.read_to_end(&mut response_body).await?;
        response_body[0..num_bytes_read].to_vec()
    };

    // Decompress if necessary
    let decompressed_body = if response_headers.get("content-encoding").is_some() {
        todo!() // Handle decompression if needed
    } else {
        response_body
    };

    // Convert the response body Vec<u8> to a string
    let response_body_str = String::from_utf8(decompressed_body)?;

    // Convert to HTTP crate response
    let mut response: Response<Res> = Response::builder()
        .status(response_status)
        .version(response_version)
        .body(response_body_str.into())?;

    // Copy response headers to response
    *response.headers_mut() = response_headers;

    // log
    log::debug!("response = {response:?}");

    Ok(response)
}
