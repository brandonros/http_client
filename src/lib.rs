#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod platform;
mod request;
mod response;
mod error;

#[cfg(feature = "std")]
mod async_connection_factory;

use alloc::vec::Vec;
use http::{Request, Response};
use platform::PlatformConnection;
use simple_error::SimpleResult;

#[cfg(feature = "std")]
use platform::std_impl::StdConnection as PlatformConnection;
#[cfg(all(target_arch = "wasm32", not(feature = "std")))]
use platform::wasm::WasmConnection as PlatformConnection;

pub struct HttpClient;

impl HttpClient {
    pub async fn request<T>(request: &Request<T>) -> SimpleResult<Response<Vec<u8>>>
    where
        T: AsRef<[u8]>,
    {
        #[cfg(feature = "std")]
        {
            let stream = async_connection_factory::AsyncConnectionFactory::connect(request).await?;
            let mut connection = PlatformConnection::new(stream).await;
            connection.send_request(request).await
        }

        #[cfg(not(feature = "std"))]
        {
            PlatformConnection::send_request(request)
        }
    }
}