use alloc::vec::Vec;
use http::{Request, Response};
use simple_error::SimpleResult;

#[cfg(feature = "std")]
use futures_lite::{AsyncRead, AsyncWrite};

#[cfg(feature = "std")]
pub mod std_impl;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(feature = "std")]
pub trait AsyncConnection: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    fn is_encrypted(&self) -> bool;
}

#[cfg(not(feature = "std"))]
pub trait Connection {
    fn is_encrypted(&self) -> bool;
}

#[cfg(feature = "std")]
pub trait PlatformConnection {
    async fn send_request<T>(
        &mut self,
        request: &Request<T>
    ) -> SimpleResult<Response<Vec<u8>>>
    where
        T: AsRef<[u8]>;
}

#[cfg(not(feature = "std"))]
pub trait PlatformConnection {
    fn send_request<T>(
        request: &Request<T>
    ) -> SimpleResult<Response<Vec<u8>>>
    where
        T: AsRef<[u8]>;
}
