//! Sending fully resolved HTTP requests: TLS, redirects, decompression, timeouts and
//! cancellation. Inputs and outputs carry no Relay model types; `relay_engine` builds a
//! [`PreparedRequest`] and reads a [`Response`].

mod client;
mod error;
mod request;
mod response;

pub use client::HttpClient;
pub use error::HttpError;
pub use request::{MultipartPart, MultipartValue, PreparedBody, PreparedRequest, RequestOptions};
pub use response::Response;
