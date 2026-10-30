//! TLS verification against a local server with a self-signed certificate (S1, S2).
//!
//! The certificate is a throwaway test fixture (`fixtures/self-signed-cert.pem`, key
//! `self-signed-key.pem`) for `127.0.0.1` only, generated with OpenSSL for this test.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the calling test"
)]

use std::io;
use std::sync::Arc;
use std::time::Duration;

use relay_http::{HttpClient, HttpError, PreparedBody, PreparedRequest, RequestOptions};
use rustls_pemfile::{certs, private_key};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_util::sync::CancellationToken;

const CERT_PEM: &str = include_str!("fixtures/self-signed-cert.pem");
const KEY_PEM: &str = include_str!("fixtures/self-signed-key.pem");

/// Starts a TLS server answering one small HTTP/1.1 response per connection and returns its
/// port.
async fn start_tls_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let certificate = certs(&mut CERT_PEM.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = private_key(&mut KEY_PEM.as_bytes()).unwrap().unwrap();
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certificate, key)
        .unwrap();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(config));
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(serve_once(acceptor, stream));
        }
    });
    port
}

/// Answers one connection with one small HTTP/1.1 response and closes it. A failed
/// connection just ends that connection; the test observes the outcome through the client.
async fn serve_once(acceptor: TlsAcceptor, stream: TcpStream) -> io::Result<()> {
    let mut tls = acceptor.accept(stream).await?;
    let mut request = Vec::new();
    // Read until the request headers end, then answer once and close.
    let mut chunk = [0u8; 1024];
    while !request.ends_with(b"\r\n\r\n") {
        let read = tls.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..read]);
    }
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
    tls.write_all(response).await?;
    tls.shutdown().await
}

fn https_request(url: String, verify_tls: bool) -> PreparedRequest {
    PreparedRequest {
        method: "GET".to_owned(),
        url,
        headers: Vec::new(),
        body: PreparedBody::None,
        options: RequestOptions {
            timeout: Duration::from_secs(10),
            follow_redirects: false,
            verify_tls,
        },
    }
}

#[tokio::test]
async fn self_signed_certificate_is_refused_with_verification() {
    let port = start_tls_server().await;
    let error = HttpClient::new()
        .send(
            https_request(format!("https://127.0.0.1:{port}/"), true),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(matches!(error, HttpError::Tls(_)), "{message}");
    assert!(message.contains("certificate"), "{message}");
}

#[tokio::test]
async fn self_signed_certificate_is_accepted_without_verification() {
    let port = start_tls_server().await;
    let response = HttpClient::new()
        .send(
            https_request(format!("https://127.0.0.1:{port}/"), false),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
}
