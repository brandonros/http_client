use std::str::FromStr;

use async_std::net::TcpStream;
use async_tls::TlsConnector;
use futures::{io::BufReader, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, Version};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

trait AsyncConn: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin> AsyncConn for T {}

fn extract_host_from_request<Req>(req: &Request<Req>) -> Result<(String, String, u16)> {
    let uri = req.uri();
    let authority = uri.authority().ok_or("No authority found in URI")?;
    let scheme = uri.scheme_str().ok_or("No scheme found in URI")?;

    // Extract host and optional port from the authority
    let host = authority.host();
    let port = authority.port_u16().unwrap_or_else(|| {
        match scheme {
            "http" => 80,   // Default port for HTTP
            "https" => 443, // Default port for HTTPS
            _ => 0,         // Indicate unknown scheme
        }
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
    let path_and_query = match uri.path_and_query() {
        Some(path_and_query) => path_and_query.as_str(),
        None => "/", // Default path if none is specified; adjust as needed
    };

    let version = match req.version() {
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1", // Default to HTTP/1.1 if uncertain
    };

    let mut request_line = format!("{} {} {}\r\n", method, path_and_query, version);

    // Add headers
    for (name, value) in req.headers() {
        request_line.push_str(&format!("{}: {}\r\n", name.as_str(), value.to_str()?));
    }

    // Add an extra line to indicate the end of the header section
    request_line.push_str("\r\n");

    // return
    Ok(request_line)
}

async fn read_response_headers<S>(reader: &mut BufReader<S>) -> Result<HeaderMap<HeaderValue>>
where
    S: AsyncReadExt + Unpin,
{
    let mut headers = HeaderMap::new();
    let mut line = String::new();

    // Read lines until an empty line is reached (headers end)
    loop {
        line.clear(); // Clear the buffer for the next line
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 || line == "\r\n" {
            break;
        }

        // Parse the header line into a key-value pair and insert it into the map
        if let Some((key, value)) = line.split_once(": ") {
            let key = key.to_lowercase();
            let value = value
                .trim_end_matches(|c: char| c == '\r' || c == '\n')
                .to_string();
            let header_name = HeaderName::from_str(&key)?;
            let header_value = HeaderValue::from_str(&value)?;
            headers.insert(header_name, header_value);
        } else {
            log::warn!("failed to parse header line {line}");
        }
    }

    Ok(headers)
}

pub async fn send_http_request<Req, Res>(request: &Request<Req>) -> Result<Response<Res>>
where
    Req: std::fmt::Debug + PartialEq<()>,
    Res: std::fmt::Debug + Sized + std::convert::From<String>,
{
    // log
    log::debug!("request = {request:?}");

    // open tcp socket
    let (scheme, host, port) = extract_host_from_request(&request)?;
    let stream = TcpStream::connect(format!("{host}:{port}")).await?;

    // optionally add tls based on scheme
    let mut stream: Box<dyn AsyncConn> = if scheme == "https" {
        let tls_connector = TlsConnector::new();
        Box::new(tls_connector.connect(&host, stream).await?)
    } else {
        Box::new(stream)
    };

    // write request
    let serialized_request = serialize_http_request(&request)?;
    stream.write_all(serialized_request.as_bytes()).await?;
    stream.flush().await?;

    // write request body if there is one
    let request_body = request.body();
    if *request_body == () {
        // no-op for empty request body
    } else {
        todo!()
    }

    // read response status
    let mut reader = BufReader::new(stream);
    let mut response_status_line = String::new();
    reader.read_line(&mut response_status_line).await?;

    // parse response status line
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

    // read response headers
    let response_headers = read_response_headers(&mut reader).await?;
    log::debug!("response_headers = {response_headers:?}");

    // read response
    let response_body = if let Some(content_length_value) = response_headers.get("content-length") {
        // read the response body based on the Content-Length
        let content_length_str = content_length_value.to_str()?;
        let content_length = content_length_str.parse::<usize>()?;
        let mut response_body = vec![0u8; content_length];
        reader.read_exact(&mut response_body).await?;
        response_body
    } else if let Some(_transfer_encoding) = response_headers.get("transfer-encoding") {
        todo!()
    } else {
        // read until end on HTTP/1.0 connection close?
        let mut response_body = Vec::with_capacity(1024 * 1024 * 8);
        let num_bytes_read = reader.read_to_end(&mut response_body).await?;
        response_body[0..num_bytes_read].to_vec()
    };

    // Decompress if necessary
    let decompressed_body =
        if let Some(_content_encoding) = response_headers.get("content-encoding") {
            todo!()
        } else {
            response_body
        };

    // convert response body vec<u8> to string
    let response_body_str = String::from_utf8(decompressed_body)?;

    // convert to http crate response
    let mut response: Response<Res> = Response::builder()
        .status(response_status)
        .version(response_version)
        .body(response_body_str.into())?;

    // copy response headers to response
    let response_headers_map = response.headers_mut();
    for (key, value) in &response_headers {
        response_headers_map.insert(key, value.clone());
    }

    // log
    log::debug!("response = {response:?}");

    Ok(response)
}
