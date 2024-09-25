use futures_lite::future;
use http::{Request, Uri};
use http_client::HttpClient;

fn main() {
    future::block_on(async {
        // init logging
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

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

        // Get the response
        let mut stream = HttpClient::connect(&request).await.expect("connect failed");
        let response = HttpClient::send::<(), String>(&mut stream, &request).await.expect("request failed");
        log::info!("response = {response:?}");
    })
}