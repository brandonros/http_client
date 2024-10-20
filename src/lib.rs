mod async_connection_factory;
mod async_connection;
mod request;
mod response;

use async_connection::AsyncConnection;
use async_connection_factory::AsyncConnectionFactory;
use futures_lite::{io::BufReader, AsyncWriteExt};
use http::{Request, Response, Uri};
use simple_error::SimpleResult;

type RequestBody = Vec<u8>;
type ResponseBody = Vec<u8>;

pub struct HttpClient;

impl HttpClient {
    pub async fn create_connection<T: std::fmt::Debug>(request: &Request<T>) -> SimpleResult<Box<dyn AsyncConnection>> {
        AsyncConnectionFactory::connect(&request).await
    }

    // Public method to send an HTTP request and return the HTTP response
    pub async fn request(stream: &mut Box<dyn AsyncConnection>, request: &Request<RequestBody>) -> SimpleResult<Response<ResponseBody>> {
        // Write the HTTP request to the stream
        let serialized_request = request::serialize_http_request(request)?;
        log::debug!("serialized_request = {serialized_request}");
        stream.write_all(serialized_request.as_bytes()).await?;
        stream.flush().await?;

        // Write request body if there is one
        if request.body().len() > 0 {
            stream.write_all(request.body()).await?;
            stream.flush().await?;
        }

        // Read and parse the response
        let mut reader = BufReader::new(stream);
        let response_status_line = response::read_response_status_line(&mut reader).await?;
        log::debug!("response_status_line = {response_status_line}");
        let (response_version, response_status) = response::parse_response_status_line(&response_status_line)?;
        let response_headers = response::read_response_headers(&mut reader).await?;
        log::debug!("response_headers = {response_headers:?}");
        let response_body = response::read_response_body(&mut reader, &response_headers).await?;
        log::debug!("response_body = {response_body:02x?}");

        // Convert to HTTP crate response
        let mut response: Response<ResponseBody> = Response::builder()
            .status(response_status)
            .version(response_version)
            .body(response_body)?;

        // Copy response headers to response
        *response.headers_mut() = response_headers;

        // log
        log::debug!("response = {response:?}");

        // return
        Ok(response)
    }

    pub async fn json_request<RequestBody, ResponseBody>(url: &str, request_body: &RequestBody) -> SimpleResult<ResponseBody>
    where 
        RequestBody: miniserde::Serialize, 
        ResponseBody: miniserde::Deserialize
    {
        // build request
        let uri: Uri = url.parse()?;
        let stringified_request_body = miniserde::json::to_string(&request_body);
        let request_body_bytes = stringified_request_body.as_bytes().to_vec();
        let request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("Content-Type", "application/json")
            .header("Content-Length", format!("{}", stringified_request_body.len()))
            .body(request_body_bytes)?;

        // make request
        let mut stream = AsyncConnectionFactory::connect(&request).await?;
        let response = Self::request(&mut stream, &request).await?;

        // parse response
        let response_body_bytes = response.body().to_owned();
        let stringified_response_body = String::from_utf8(response_body_bytes)?;
        let response_body: ResponseBody = miniserde::json::from_str(&stringified_response_body)?;

        // return
        Ok(response_body)
    }
}
