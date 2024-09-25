use futures_lite::future;
use http::{Request, Response, Uri};
use http_client::HttpClient;

fn main() {
    future::block_on(async {
        // init logging
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
        
        // Build the URI for the request
        let uri: Uri = "wss://data.tradingview.com/socket.io/websocket?from=chart%2F&date=2024_09_25-14_09&type=chart".parse().expect("Failed to parse URI");

        // Build the GET request
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .header("User-Agent", "http_client/1.0")
            .header("Host", "data.tradingview.com")
            .header("Origin", "https://www.tradingview.com")            
            .header("Connection", "upgrade")
            .header("Upgrade", "websocket")      
            .header("Sec-WebSocket-Version", "13")                        
            .header("Sec-WebSocket-Key", "0+Ob/WaxyeAN+DCZDyHWbw==")                                    
            .body(())
            .expect("Failed to build request");

        // Get the response
        let response: Response<String> = HttpClient::send(&request).await.expect("request failed");
        log::info!("response = {response:?}");
    })
}