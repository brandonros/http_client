use alloc::vec::Vec;
use http::{Request, Response, StatusCode};
use simple_error::SimpleResult;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{RequestInit, window};

use super::PlatformConnection;

pub struct WasmConnection;

impl PlatformConnection for WasmConnection {
    async fn send_request<T>(request: &Request<T>) -> SimpleResult<Response<Vec<u8>>>
    where
        T: AsRef<[u8]>,
    {
        // Create RequestInit object
        let mut opts = RequestInit::new();
        opts.method(request.method().as_str());
        opts.mode(web_sys::RequestMode::Cors);

        // Set body if present
        if request.method() != http::Method::GET {
            // Convert body to Uint8Array or similar
            let body = js_sys::Uint8Array::from(request.body().as_ref());
            opts.body(Some(&body));
        }

        // Create the request
        let url = request.uri().to_string();
        let web_request = web_sys::Request::new_with_str_and_init(&url, &opts)?;

        // Set headers
        let web_headers = web_request.headers();
        for (name, value) in request.headers() {
            web_headers.set(name.as_str(), value.to_str()?)?;
        }

        // Perform fetch
        let window = window().ok_or("Failed to get window")?;
        let resp_value = JsFuture::from(window.fetch_with_request(&web_request)).await?;
        let web_resp: web_sys::Response = resp_value.dyn_into()?;

        // Get status
        let status = StatusCode::from_u16(web_resp.status())?;

        // Get headers
        let web_headers = web_resp.headers();
        let mut headers = http::HeaderMap::new();
        // ... populate headers from web_headers ...

        // Get body
        let array_buffer = JsFuture::from(web_resp.array_buffer()?).await?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let mut body = vec![0; uint8_array.length() as usize];
        uint8_array.copy_to(&mut body);

        // Build response
        let mut response = Response::builder()
            .status(status)
            .body(body)?;
        *response.headers_mut() = headers;

        Ok(response)
    }
}