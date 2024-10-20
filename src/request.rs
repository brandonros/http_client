use http::{Request, Version};
use simple_error::SimpleResult;

// Serializes the HTTP request into a string format that can be sent over the network
pub fn serialize_http_request<T>(req: &Request<T>) -> SimpleResult<String> {
    let method = req.method();
    let uri = req.uri();

    let path_and_query = uri.path_and_query().map_or("/", |pq| pq.as_str());

    let version = match req.version() {
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };

    let mut request_line = format!("{method} {path_and_query} {version}\r\n");

    for (name, value) in req.headers() {
        request_line.push_str(&format!("{}: {}\r\n", name.as_str(), value.to_str()?));
    }

    request_line.push_str("\r\n");

    Ok(request_line)
}
