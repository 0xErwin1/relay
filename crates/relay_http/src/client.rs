//! The client that sends a [`PreparedRequest`].

use std::collections::HashMap;
use std::error::Error as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, RequestBuilder, Version, redirect};
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::error::HttpError;
use crate::request::{MultipartValue, PreparedBody, PreparedRequest, RequestOptions};
use crate::response::Response;

/// The maximum number of redirects followed for one request.
const MAX_REDIRECTS: usize = 10;

/// Sends requests, reusing one `reqwest` client per (verify TLS, follow redirects) pair.
///
/// Cloning is cheap and clones share the cache. Requests must run inside a Tokio runtime.
#[derive(Debug, Clone, Default)]
pub struct HttpClient {
    clients: Arc<Mutex<HashMap<ClientKey, reqwest::Client>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClientKey {
    verify_tls: bool,
    follow_redirects: bool,
}

impl HttpClient {
    /// A client with an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends `request` and reads the whole response body.
    ///
    /// Cancelling `cancel` aborts only this request with [`HttpError::Cancelled`].
    pub async fn send(
        &self,
        request: PreparedRequest,
        cancel: CancellationToken,
    ) -> Result<Response, HttpError> {
        let timeout = request.options.timeout;
        tokio::select! {
            biased;
            () = cancel.cancelled() => Err(HttpError::Cancelled),
            result = tokio::time::timeout(timeout, self.execute(request)) => {
                result.unwrap_or(Err(HttpError::Timeout(timeout.as_millis())))
            }
        }
    }

    async fn execute(&self, request: PreparedRequest) -> Result<Response, HttpError> {
        let url = Url::parse(&request.url).map_err(|error| HttpError::InvalidUrl {
            url: request.url.clone(),
            reason: error.to_string(),
        })?;
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|_| HttpError::InvalidMethod(request.method.clone()))?;
        let headers = header_map(&request.headers)?;
        let has_content_type = headers.contains_key(CONTENT_TYPE);
        let client = self.client(request.options)?;
        let builder = client.request(method, url.clone()).headers(headers);
        let builder = with_body(builder, request.body, has_content_type).await?;

        let started = Instant::now();
        let timeout = request.options.timeout;
        let response = builder
            .send()
            .await
            .map_err(|error| classify(&error, &url, timeout))?;
        let status = response.status();
        let version = version_text(response.version());
        let final_url = response.url().to_string();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let body = response
            .bytes()
            .await
            .map_err(|error| classify(&error, &url, timeout))?;
        Ok(Response {
            status: status.as_u16(),
            reason: status.canonical_reason().map(str::to_owned),
            version,
            headers,
            body: body.to_vec(),
            final_url,
            elapsed: started.elapsed(),
        })
    }

    fn client(&self, options: RequestOptions) -> Result<reqwest::Client, HttpError> {
        let key = ClientKey {
            verify_tls: options.verify_tls,
            follow_redirects: options.follow_redirects,
        };
        let mut clients = self.clients.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }
        let policy = if key.follow_redirects {
            redirect::Policy::limited(MAX_REDIRECTS)
        } else {
            redirect::Policy::none()
        };
        let client = reqwest::Client::builder()
            .redirect(policy)
            .tls_danger_accept_invalid_certs(!key.verify_tls)
            .build()
            .map_err(|error| HttpError::Client(error_chain(&error)))?;
        clients.insert(key, client.clone());
        Ok(client)
    }
}

fn header_map(headers: &[(String, String)]) -> Result<HeaderMap, HttpError> {
    let mut map = HeaderMap::with_capacity(headers.len());
    for (name, value) in headers {
        let invalid = |reason: String| HttpError::InvalidHeader {
            name: name.clone(),
            reason,
        };
        let header_name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| invalid(error.to_string()))?;
        let header_value =
            HeaderValue::from_str(value).map_err(|error| invalid(error.to_string()))?;
        map.append(header_name, header_value);
    }
    Ok(map)
}

async fn with_body(
    builder: RequestBuilder,
    body: PreparedBody,
    has_content_type: bool,
) -> Result<RequestBuilder, HttpError> {
    Ok(match body {
        PreparedBody::None => builder,
        PreparedBody::Bytes { content_type, data } => {
            let builder = match content_type {
                Some(content_type) if !has_content_type => {
                    builder.header(CONTENT_TYPE, content_type)
                }
                _ => builder,
            };
            builder.body(data)
        }
        PreparedBody::Form(fields) => builder.form(&fields),
        PreparedBody::Multipart(parts) => {
            let mut form = Form::new();
            for part in parts {
                form = match part.value {
                    MultipartValue::Text(text) => form.text(part.name, text),
                    MultipartValue::File(path) => {
                        let file = Part::file(&path)
                            .await
                            .map_err(|source| HttpError::File { path, source })?;
                        form.part(part.name, file)
                    }
                };
            }
            builder.multipart(form)
        }
        PreparedBody::File(path) => {
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|source| HttpError::File { path, source })?;
            builder.body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
        }
    })
}

/// Maps a `reqwest` failure to the S2 messages.
fn classify(error: &reqwest::Error, url: &Url, timeout: Duration) -> HttpError {
    if error.is_timeout() {
        return HttpError::Timeout(timeout.as_millis());
    }
    if error.is_connect() {
        if is_tls(error) {
            return HttpError::Tls(root_cause(error));
        }
        return HttpError::Connect {
            host: url.host_str().unwrap_or_default().to_owned(),
            reason: root_cause(error),
        };
    }
    HttpError::Request(error_chain(error))
}

/// Whether the error chain comes from the TLS layer. `reqwest` does not expose a typed TLS
/// error, so this matches the rustls messages, which always name a certificate or TLS.
fn is_tls(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(current) = source {
        let message = current.to_string().to_lowercase();
        if message.contains("certificate") || message.contains("tls") {
            return true;
        }
        source = current.source();
    }
    false
}

/// The message of the innermost error, which carries the actual reason.
fn root_cause(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(current) = source {
        message = current.to_string();
        source = current.source();
    }
    message
}

/// The error and its sources joined with `: `.
fn error_chain(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(current) = source {
        message.push_str(": ");
        message.push_str(&current.to_string());
        source = current.source();
    }
    message
}

fn version_text(version: Version) -> String {
    match version {
        Version::HTTP_09 => "0.9",
        Version::HTTP_10 => "1.0",
        Version::HTTP_11 => "1.1",
        Version::HTTP_2 => "2",
        Version::HTTP_3 => "3",
        _ => "unknown",
    }
    .to_owned()
}
