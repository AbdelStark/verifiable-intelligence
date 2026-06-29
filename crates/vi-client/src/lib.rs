//! HTTPS client for the `CommitLLM`-instrumented provider endpoint.
//!
//! Filled in by RFC-0006 implementation issues.

// RFC-0006 client failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use std::env;

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Url,
};
use vi_errors::{NetworkErrorKind, ViError};

/// Environment variable containing the provider bearer token.
pub const API_KEY_ENV: &str = "VI_API_KEY";

/// Trace header propagated to provider chat and audit calls.
pub const TRACE_HEADER: &str = "X-Verifiable-Intelligence-Trace";

/// Receipt opt-in header from RFC-0006.
pub const RECEIPT_HEADER: &str = "X-Verifiable-Receipt";

/// Provider chat-completions path.
pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// HTTPS provider client for receipt-enabled chat requests.
#[derive(Debug, Clone)]
pub struct ChatClient {
    endpoint: Url,
    api_key: Option<String>,
    client: Client,
}

impl ChatClient {
    /// Construct a client from `endpoint` and optional bearer token.
    pub fn new(endpoint: impl AsRef<str>, api_key: Option<String>) -> Result<Self, ViError> {
        let endpoint = parse_https_endpoint(endpoint.as_ref())?;
        Ok(Self {
            endpoint,
            api_key,
            client: Client::new(),
        })
    }

    /// Construct a client, reading `VI_API_KEY` when present.
    pub fn from_env(endpoint: impl AsRef<str>) -> Result<Self, ViError> {
        Self::new(endpoint, env::var(API_KEY_ENV).ok())
    }

    /// POST an OpenAI-compatible chat-completions JSON body with receipt opt-in.
    pub fn post_chat_completions(
        &self,
        trace_id: &str,
        json_body: impl Into<String>,
    ) -> Result<ChatResponse, ViError> {
        let url = self.chat_url()?;
        let response = self
            .client
            .post(url.clone())
            .headers(self.chat_headers(trace_id)?)
            .body(json_body.into())
            .send()
            .map_err(|error| network_error(url.as_str(), &error))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        if !status.is_success() {
            return Err(ViError::Network {
                endpoint: url.to_string(),
                kind: NetworkErrorKind::HttpStatus,
                http_status: Some(status.as_u16()),
            });
        }

        let body = response
            .bytes()
            .map_err(|error| network_error(url.as_str(), &error))?
            .to_vec();
        Ok(ChatResponse {
            status: status.as_u16(),
            content_type,
            body,
        })
    }

    fn chat_url(&self) -> Result<Url, ViError> {
        self.endpoint
            .join(CHAT_COMPLETIONS_PATH)
            .map_err(|error| input_error("endpoint", format!("invalid chat path: {error}")))
    }

    fn chat_headers(&self, trace_id: &str) -> Result<HeaderMap, ViError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(RECEIPT_HEADER, HeaderValue::from_static("1"));
        headers.insert(
            TRACE_HEADER,
            HeaderValue::from_str(trace_id).map_err(|error| {
                input_error("trace_id", format!("invalid trace header value: {error}"))
            })?,
        );

        if let Some(api_key) = &self.api_key {
            let mut value =
                HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|error| {
                    input_error(
                        "VI_API_KEY",
                        format!("invalid authorization header: {error}"),
                    )
                })?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }

        Ok(headers)
    }
}

/// Opaque response body for the later multipart parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response content type, when present.
    pub content_type: Option<String>,
    /// Raw response bytes.
    pub body: Vec<u8>,
}

pub fn placeholder() {}

fn parse_https_endpoint(endpoint: &str) -> Result<Url, ViError> {
    let url = Url::parse(endpoint)
        .map_err(|error| input_error("endpoint", format!("invalid provider URL: {error}")))?;
    if url.scheme() != "https" {
        return Err(input_error("endpoint", "provider endpoint must use https"));
    }
    Ok(url)
}

fn input_error(arg: impl Into<String>, reason: impl Into<String>) -> ViError {
    ViError::Input {
        arg: arg.into(),
        reason: reason.into(),
        detail: None,
    }
}

fn network_error(endpoint: &str, error: &reqwest::Error) -> ViError {
    let message = error.to_string().to_ascii_lowercase();
    let kind = if error.is_timeout() {
        NetworkErrorKind::Timeout
    } else if message.contains("eof") {
        NetworkErrorKind::TlsHandshakeEof
    } else if message.contains("tls")
        || message.contains("certificate")
        || message.contains("handshake")
    {
        NetworkErrorKind::Tls
    } else if error.is_connect() {
        NetworkErrorKind::ConnectionRefused
    } else {
        NetworkErrorKind::Other
    };

    ViError::Network {
        endpoint: endpoint.to_owned(),
        kind,
        http_status: error.status().map(|status| status.as_u16()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        io::{Read, Write},
        net::TcpListener,
        sync::Mutex,
        thread,
    };

    use reqwest::header::AUTHORIZATION;
    use vi_errors::ViError;

    use super::{ChatClient, API_KEY_ENV, RECEIPT_HEADER, TRACE_HEADER};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn http_endpoint_is_input_error() {
        let error = ChatClient::new("http://provider.example", None)
            .expect_err("http endpoint should fail");

        assert_eq!(
            error,
            ViError::Input {
                arg: "endpoint".to_owned(),
                reason: "provider endpoint must use https".to_owned(),
                detail: None,
            }
        );
    }

    #[test]
    fn api_key_env_is_used_as_sensitive_authorization_header() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = env::var_os(API_KEY_ENV);
        env::set_var(API_KEY_ENV, "secret-token");

        let client = ChatClient::from_env("https://provider.example").expect("client builds");
        let headers = client.chat_headers("trace-123").expect("headers build");

        restore_env(previous);
        assert_eq!(headers.get(RECEIPT_HEADER).expect("receipt header"), "1");
        assert_eq!(
            headers.get(TRACE_HEADER).expect("trace header"),
            "trace-123"
        );
        assert_eq!(
            headers.get(AUTHORIZATION).expect("auth header"),
            "Bearer secret-token"
        );
        assert!(!format!("{headers:?}").contains("secret-token"));
    }

    #[test]
    fn tls_failure_is_network_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener binds");
        let addr = listener.local_addr().expect("listener addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("connection accepted");
            let mut buf = [0_u8; 64];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        });
        let client = ChatClient::new(format!("https://{addr}"), None).expect("https client builds");

        let error = client
            .post_chat_completions("trace", "{}")
            .expect_err("plain HTTP server should fail TLS");
        handle.join().expect("server joins");

        assert_eq!(error.category(), "network");
    }

    fn restore_env(previous: Option<std::ffi::OsString>) {
        if let Some(previous) = previous {
            env::set_var(API_KEY_ENV, previous);
        } else {
            env::remove_var(API_KEY_ENV);
        }
    }
}
