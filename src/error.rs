use http::header::{InvalidHeaderName, InvalidHeaderValue, ToStrError};
use simple_error::{SimpleError, SimpleResult};

pub trait IntoSimpleError<T> {
    fn into_simple_error(self) -> SimpleResult<T>;
}

impl<T> IntoSimpleError<T> for Result<T, http::Error> {
    fn into_simple_error(self) -> SimpleResult<T> {
        self.map_err(|e| SimpleError::new(e.to_string()))
    }
}

impl<T> IntoSimpleError<T> for Result<T, InvalidHeaderName> {
    fn into_simple_error(self) -> SimpleResult<T> {
        self.map_err(|e| SimpleError::new(e.to_string()))
    }
}

impl<T> IntoSimpleError<T> for Result<T, InvalidHeaderValue> {
    fn into_simple_error(self) -> SimpleResult<T> {
        self.map_err(|e| SimpleError::new(e.to_string()))
    }
}

impl<T> IntoSimpleError<T> for Result<T, ToStrError> {
    fn into_simple_error(self) -> SimpleResult<T> {
        self.map_err(|e| SimpleError::new(e.to_string()))
    }
} 