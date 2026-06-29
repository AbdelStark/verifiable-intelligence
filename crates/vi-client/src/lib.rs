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
use serde_json::Value;
use vi_errors::{NetworkErrorKind, ViError};

/// Environment variable containing the provider bearer token.
pub const API_KEY_ENV: &str = "VI_API_KEY";

/// Trace header propagated to provider chat and audit calls.
pub const TRACE_HEADER: &str = "X-Verifiable-Intelligence-Trace";

/// Receipt opt-in header from RFC-0006.
pub const RECEIPT_HEADER: &str = "X-Verifiable-Receipt";

/// Provider chat-completions path.
pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

const RECEIPT_CONTENT_TYPE: &str = "application/vnd.verifiable-intelligence.receipt+binary";
const MULTIPART_MIXED: &str = "multipart/mixed";

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
        let warning = warning_199(response.headers());

        if !status.is_success() && warning.is_none() {
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
            endpoint: url.to_string(),
            content_type,
            warning,
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
    /// Endpoint URL that produced this response.
    pub endpoint: String,
    /// Response content type, when present.
    pub content_type: Option<String>,
    /// Degraded provider warning, when present.
    pub warning: Option<String>,
    /// Raw response bytes.
    pub body: Vec<u8>,
}

/// Parsed chat response with optional receipt bytes for degraded responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChatResponse {
    /// Assistant text from the chat-completion JSON.
    pub text: String,
    /// Binary `VIRC` receipt bytes, absent only for degraded `Warning: 199` responses.
    pub receipt_bytes: Option<Vec<u8>>,
    /// Degraded provider warning, when present.
    pub warning: Option<String>,
}

/// Parse an RFC-0006 chat response into text plus receipt bytes.
pub fn parse_chat_response(response: &ChatResponse) -> Result<ParsedChatResponse, ViError> {
    let content_type = response.content_type.as_deref().unwrap_or("");
    if response.warning.is_some() {
        return Ok(ParsedChatResponse {
            text: chat_text_from_json(&response.body)?,
            receipt_bytes: None,
            warning: response.warning.clone(),
        });
    }

    if content_type
        .to_ascii_lowercase()
        .starts_with(MULTIPART_MIXED)
    {
        parse_multipart_chat_response(response)
    } else {
        Err(ViError::ReceiptMissing {
            endpoint: response.endpoint.clone(),
            content_type: content_type.to_owned(),
        })
    }
}

pub fn placeholder() {}

fn parse_multipart_chat_response(response: &ChatResponse) -> Result<ParsedChatResponse, ViError> {
    let content_type = response.content_type.as_deref().unwrap_or("");
    let boundary = multipart_boundary(content_type)?;
    let parts = parse_multipart(&response.body, &boundary)?;
    let json_part = parts
        .iter()
        .find(|part| {
            part.content_type
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .starts_with("application/json")
        })
        .ok_or_else(|| corrupt_multipart(0, "multipart response missing JSON part"))?;
    let receipt_part = parts
        .iter()
        .find(|part| {
            part.content_type
                .as_deref()
                .unwrap_or("")
                .eq_ignore_ascii_case(RECEIPT_CONTENT_TYPE)
        })
        .ok_or_else(|| ViError::ReceiptMissing {
            endpoint: response.endpoint.clone(),
            content_type: content_type.to_owned(),
        })?;

    Ok(ParsedChatResponse {
        text: chat_text_from_json(&json_part.body)?,
        receipt_bytes: Some(receipt_part.body.clone()),
        warning: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MultipartPart {
    content_type: Option<String>,
    body: Vec<u8>,
}

fn multipart_boundary(content_type: &str) -> Result<String, ViError> {
    for parameter in content_type.split(';').skip(1) {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("boundary") {
            let boundary = value.trim().trim_matches('"');
            if boundary.is_empty() {
                return Err(corrupt_multipart(0, "multipart boundary is empty"));
            }
            return Ok(boundary.to_owned());
        }
    }

    Err(corrupt_multipart(
        0,
        "multipart content-type missing boundary",
    ))
}

fn parse_multipart(body: &[u8], boundary: &str) -> Result<Vec<MultipartPart>, ViError> {
    let delimiter = format!("--{boundary}").into_bytes();
    if !body.starts_with(&delimiter) {
        return Err(corrupt_multipart(
            0,
            "multipart body missing opening boundary",
        ));
    }

    let mut parts = Vec::new();
    let mut cursor = delimiter.len();
    loop {
        if body.get(cursor..cursor + 2) == Some(b"--") {
            return Ok(parts);
        }
        if body.get(cursor..cursor + 2) != Some(b"\r\n") {
            return Err(corrupt_multipart(cursor, "multipart boundary missing CRLF"));
        }
        cursor += 2;

        let header_end = find_subslice(&body[cursor..], b"\r\n\r\n")
            .map(|offset| cursor + offset)
            .ok_or_else(|| corrupt_multipart(cursor, "multipart part missing header terminator"))?;
        let headers = parse_part_headers(&body[cursor..header_end])?;
        let body_start = header_end + 4;
        let next_marker = find_subslice(&body[body_start..], b"\r\n--")
            .map(|offset| body_start + offset)
            .ok_or_else(|| corrupt_multipart(body_start, "multipart part missing next boundary"))?;
        if body.get(next_marker + 4..next_marker + 4 + boundary.len()) != Some(boundary.as_bytes())
        {
            return Err(corrupt_multipart(
                next_marker,
                "multipart part boundary mismatch",
            ));
        }

        let part_body = body[body_start..next_marker].to_vec();
        let content_type = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.clone());
        parts.push(MultipartPart {
            content_type,
            body: part_body,
        });
        cursor = next_marker + 4 + boundary.len();
    }
}

fn parse_part_headers(bytes: &[u8]) -> Result<Vec<(String, String)>, ViError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| corrupt_multipart(0, "multipart headers are not UTF-8"))?;
    let mut headers = Vec::new();
    for line in text.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            return Err(corrupt_multipart(0, "malformed multipart header"));
        };
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(headers)
}

fn chat_text_from_json(bytes: &[u8]) -> Result<String, ViError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| corrupt_multipart(0, "chat-completion JSON is malformed"))?;
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| corrupt_multipart(0, "chat-completion JSON missing message content"))
}

fn warning_199(headers: &HeaderMap) -> Option<String> {
    headers.get_all("warning").iter().find_map(|value| {
        let value = value.to_str().ok()?;
        value
            .trim_start()
            .starts_with("199")
            .then(|| value.to_owned())
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn corrupt_multipart(offset: usize, reason: &'static str) -> ViError {
    ViError::CorruptEnvelope {
        envelope: "multipart/mixed",
        offset,
        reason,
    }
}

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

    use super::{
        parse_chat_response, ChatClient, ChatResponse, API_KEY_ENV, RECEIPT_HEADER, TRACE_HEADER,
    };

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

    #[test]
    fn multipart_with_json_and_receipt_parses() {
        let response = multipart_response(&[
            (
                "application/json",
                br#"{"choices":[{"message":{"content":"hello"}}]}"#.as_slice(),
            ),
            (
                "application/vnd.verifiable-intelligence.receipt+binary",
                b"VIRC-receipt-bytes".as_slice(),
            ),
        ]);

        let parsed = parse_chat_response(&response).expect("multipart should parse");

        assert_eq!(parsed.text, "hello");
        assert_eq!(
            parsed.receipt_bytes.as_deref(),
            Some(b"VIRC-receipt-bytes".as_slice())
        );
        assert_eq!(parsed.warning, None);
    }

    #[test]
    fn multipart_without_receipt_is_receipt_missing() {
        let response = multipart_response(&[(
            "application/json",
            br#"{"choices":[{"message":{"content":"hello"}}]}"#.as_slice(),
        )]);

        let error = parse_chat_response(&response).expect_err("receipt should be required");

        assert_eq!(
            error,
            ViError::ReceiptMissing {
                endpoint: "https://provider.example/v1/chat/completions".to_owned(),
                content_type: "multipart/mixed; boundary=test-boundary".to_owned(),
            }
        );
    }

    #[test]
    fn malformed_multipart_is_corrupt_envelope() {
        let response = ChatResponse {
            status: 200,
            endpoint: "https://provider.example/v1/chat/completions".to_owned(),
            content_type: Some("multipart/mixed; boundary=test-boundary".to_owned()),
            warning: None,
            body: b"--wrong-boundary\r\n".to_vec(),
        };

        let error = parse_chat_response(&response).expect_err("malformed multipart should fail");

        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "multipart/mixed",
                offset: 0,
                reason: "multipart body missing opening boundary",
            }
        );
    }

    #[test]
    fn warning_199_is_surfaced_without_receipt() {
        let response = ChatResponse {
            status: 503,
            endpoint: "https://provider.example/v1/chat/completions".to_owned(),
            content_type: Some("application/json".to_owned()),
            warning: Some(r#"199 - "Receipt unavailable""#.to_owned()),
            body: br#"{"choices":[{"message":{"content":"degraded text"}}]}"#.to_vec(),
        };

        let parsed = parse_chat_response(&response).expect("warning response should parse");

        assert_eq!(parsed.text, "degraded text");
        assert_eq!(parsed.receipt_bytes, None);
        assert_eq!(
            parsed.warning.as_deref(),
            Some(r#"199 - "Receipt unavailable""#)
        );
    }

    fn restore_env(previous: Option<std::ffi::OsString>) {
        if let Some(previous) = previous {
            env::set_var(API_KEY_ENV, previous);
        } else {
            env::remove_var(API_KEY_ENV);
        }
    }

    fn multipart_response(parts: &[(&str, &[u8])]) -> ChatResponse {
        let mut body = Vec::new();
        for (content_type, part_body) in parts {
            body.extend_from_slice(b"--test-boundary\r\nContent-Type: ");
            body.extend_from_slice(content_type.as_bytes());
            body.extend_from_slice(b"\r\n\r\n");
            body.extend_from_slice(part_body);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--test-boundary--\r\n");

        ChatResponse {
            status: 200,
            endpoint: "https://provider.example/v1/chat/completions".to_owned(),
            content_type: Some("multipart/mixed; boundary=test-boundary".to_owned()),
            warning: None,
            body,
        }
    }
}
