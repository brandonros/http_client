use futures_lite::future;
use http_client::send_http_request;
use http::{Request, Response, Uri};

fn main() {
    future::block_on(async {
        // Build the URI for the request
        let uri: Uri = "https://www.google.com/".parse().expect("Failed to parse URI");

        // Build the GET request
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .header("User-Agent", "http_client/1.0")
            .header("Host", "www.google.com")
            .body(())
            .expect("Failed to build request");

        let response: Response<String> = send_http_request(&request).await.expect("Failed to send request");

        println!("{response:?}");
    })
}