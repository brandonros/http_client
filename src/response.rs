use std::str::FromStr;

use futures_lite::{io::BufReader, AsyncBufReadExt, AsyncRead, AsyncReadExt};
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Version};
use simple_error::{box_err, SimpleResult};

// Reads the response status line from the stream
pub async fn read_response_status_line<S>(reader: &mut BufReader<S>) -> SimpleResult<String>
where
    S: AsyncRead + Unpin,
{
    let mut response_status_line = String::new();
    reader.read_line(&mut response_status_line).await?;
    Ok(response_status_line)
}

// Parses the response status line into a version and status code
pub fn parse_response_status_line(response_status_line: &str) -> SimpleResult<(Version, StatusCode)> {
    let response_status_line_parts: Vec<&str> =
        response_status_line.split_whitespace().collect();
    if response_status_line_parts.len() < 2 {
        return Err(box_err!("Failed to parse response status line"));
    }

    let response_version = match response_status_line_parts[0] {
        "HTTP/1.0" => Version::HTTP_10,
        "HTTP/1.1" => Version::HTTP_11,
        "HTTP/2.0" => Version::HTTP_2,
        _ => return Err(box_err!("Unsupported HTTP version")),
    };

    let response_status = StatusCode::from_u16(response_status_line_parts[1].parse()?)?;
    Ok((response_version, response_status))
}

// Reads the response headers from the provided BufReader
pub async fn read_response_headers<S>(reader: &mut BufReader<S>) -> SimpleResult<HeaderMap<HeaderValue>>
where
    S: AsyncRead + Unpin,
{
    let mut headers = HeaderMap::new();
    let mut line = String::new();

    while reader.read_line(&mut line).await? != 0 && line != "\r\n" {
        if let Some((key, value)) = line.split_once(": ") {
            let key = key.to_lowercase();
            let value = value.trim_end_matches(|c: char| c == '\r' || c == '\n');
            let header_name = HeaderName::from_str(&key)?;
            let header_value = HeaderValue::from_str(value)?;
            headers.insert(header_name, header_value);
        } else {
            log::warn!("Failed to parse header line: {line}");
        }
        line.clear();
    }

    Ok(headers)
}

// Reads a chunked HTTP body from the provided BufReader
pub async fn read_chunked_body<S>(reader: &mut BufReader<S>) -> SimpleResult<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut body = Vec::new();
    let mut chunk_size_line = String::new();

    loop {
        reader.read_line(&mut chunk_size_line).await?;
        let chunk_size = usize::from_str_radix(chunk_size_line.trim(), 16)?;

        if chunk_size == 0 {
            break;
        }

        let mut chunk = vec![0; chunk_size];
        reader.read_exact(&mut chunk).await?;
        body.extend_from_slice(&chunk);

        let mut crlf = [0; 2];
        reader.read_exact(&mut crlf).await?;
        if &crlf != b"\r\n" {
            return Err(box_err!("Invalid chunked encoding: missing CRLF"));
        }
        chunk_size_line.clear();
    }

    Ok(body)
}

// Reads the response body based on headers
pub async fn read_response_body<S>(
    reader: &mut BufReader<S>,
    headers: &HeaderMap<HeaderValue>,
) -> SimpleResult<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    if let Some(content_length_value) = headers.get("content-length") {
        let content_length = content_length_value.to_str()?.parse::<usize>()?;
        let mut response_body = vec![0u8; content_length];
        reader.read_exact(&mut response_body).await?;
        Ok(response_body)
    } else if let Some(transfer_encoding) = headers.get("transfer-encoding") {
        if transfer_encoding == "chunked" {
            read_chunked_body(reader).await
        } else {
            todo!() // Handle other transfer encodings if needed
        }
    } else if let Some(connection) = headers.get("connection") {
        if connection == "upgrade" || connection == "Upgrade" {
            Ok(vec![]) // assume empty response body on websocket upgrade
        } else {
            todo!()
        }
    } else {
        let mut response_body = Vec::with_capacity(8 * 1024 * 1024);
        let num_bytes_read = reader.read_to_end(&mut response_body).await?;
        Ok(response_body[0..num_bytes_read].to_vec())
    }
}
