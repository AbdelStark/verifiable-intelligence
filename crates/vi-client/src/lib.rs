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
use vi_receipt::AuditChallenge;

/// Environment variable containing the provider bearer token.
pub const API_KEY_ENV: &str = "VI_API_KEY";

/// Trace header propagated to provider chat and audit calls.
pub const TRACE_HEADER: &str = "X-Verifiable-Intelligence-Trace";

/// Receipt opt-in header from RFC-0006.
pub const RECEIPT_HEADER: &str = "X-Verifiable-Receipt";

/// Provider chat-completions path.
pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// Provider audit path.
pub const AUDIT_PATH: &str = "/v1/audit";

/// Provider health preflight path.
pub const HEALTHZ_PATH: &str = "/healthz";

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
        self.post_chat_completions_inner(trace_id, json_body, true)
    }

    /// POST an OpenAI-compatible chat-completions JSON body without receipt opt-in.
    pub fn post_chat_completions_without_receipt(
        &self,
        trace_id: &str,
        json_body: impl Into<String>,
    ) -> Result<ChatResponse, ViError> {
        self.post_chat_completions_inner(trace_id, json_body, false)
    }

    fn post_chat_completions_inner(
        &self,
        trace_id: &str,
        json_body: impl Into<String>,
        receipt_opt_in: bool,
    ) -> Result<ChatResponse, ViError> {
        let url = self.chat_url()?;
        let response = self
            .client
            .post(url.clone())
            .headers(self.chat_headers_with_receipt(trace_id, receipt_opt_in)?)
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

    /// POST a verifier audit challenge and return the raw `VIAU` envelope bytes.
    pub fn post_audit(
        &self,
        trace_id: &str,
        request: &AuditRequest,
    ) -> Result<AuditResponse, ViError> {
        let url = self.audit_url()?;
        let response = self
            .client
            .post(url.clone())
            .headers(self.audit_headers(trace_id)?)
            .body(request.to_json_body())
            .send()
            .map_err(|error| network_error(url.as_str(), &error))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        if !status.is_success() {
            return Err(http_status_error(url.as_str(), status.as_u16()));
        }

        let body = response
            .bytes()
            .map_err(|error| network_error(url.as_str(), &error))?
            .to_vec();
        Ok(AuditResponse {
            status: status.as_u16(),
            endpoint: url.to_string(),
            content_type,
            body,
        })
    }

    /// GET provider health metadata from `/healthz`.
    pub fn get_healthz(&self) -> Result<ProviderHealth, ViError> {
        let url = self.healthz_url()?;
        let response = self
            .client
            .get(url.clone())
            .headers(self.auth_headers()?)
            .send()
            .map_err(|error| network_error(url.as_str(), &error))?;
        let status = response.status();

        if !status.is_success() {
            return Err(http_status_error(url.as_str(), status.as_u16()));
        }

        let body = response
            .bytes()
            .map_err(|error| network_error(url.as_str(), &error))?;
        parse_provider_health(&body)
    }

    /// Optional preflight comparing provider `/healthz` pin with the local pin.
    pub fn preflight_commitllm_pin(
        &self,
        expected_short_pin: &str,
    ) -> Result<HealthPreflight, ViError> {
        let healthz_url = self.healthz_url()?.to_string();
        let health = self.get_healthz()?;
        Ok(compare_commitllm_pin(
            health,
            expected_short_pin,
            &healthz_url,
        ))
    }

    fn chat_url(&self) -> Result<Url, ViError> {
        self.endpoint
            .join(CHAT_COMPLETIONS_PATH)
            .map_err(|error| input_error("endpoint", format!("invalid chat path: {error}")))
    }

    fn audit_url(&self) -> Result<Url, ViError> {
        self.endpoint
            .join(AUDIT_PATH)
            .map_err(|error| input_error("endpoint", format!("invalid audit path: {error}")))
    }

    fn healthz_url(&self) -> Result<Url, ViError> {
        self.endpoint
            .join(HEALTHZ_PATH)
            .map_err(|error| input_error("endpoint", format!("invalid healthz path: {error}")))
    }

    fn chat_headers_with_receipt(
        &self,
        trace_id: &str,
        receipt_opt_in: bool,
    ) -> Result<HeaderMap, ViError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if receipt_opt_in {
            headers.insert(RECEIPT_HEADER, HeaderValue::from_static("1"));
        }
        headers.insert(
            TRACE_HEADER,
            HeaderValue::from_str(trace_id).map_err(|error| {
                input_error("trace_id", format!("invalid trace header value: {error}"))
            })?,
        );
        self.insert_authorization(&mut headers)?;

        Ok(headers)
    }

    fn audit_headers(&self, trace_id: &str) -> Result<HeaderMap, ViError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            TRACE_HEADER,
            HeaderValue::from_str(trace_id).map_err(|error| {
                input_error("trace_id", format!("invalid trace header value: {error}"))
            })?,
        );
        self.insert_authorization(&mut headers)?;

        Ok(headers)
    }

    fn auth_headers(&self) -> Result<HeaderMap, ViError> {
        let mut headers = HeaderMap::new();
        self.insert_authorization(&mut headers)?;
        Ok(headers)
    }

    fn insert_authorization(&self, headers: &mut HeaderMap) -> Result<(), ViError> {
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

        Ok(())
    }

    #[cfg(test)]
    fn new_unchecked(endpoint: Url, api_key: Option<String>) -> Self {
        Self {
            endpoint,
            api_key,
            client: Client::new(),
        }
    }
}

/// Provider metadata returned by `/healthz`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealth {
    /// Provider health status, normally `ok`.
    pub status: String,
    /// Public model identifier.
    pub model_id: String,
    /// Canonical checkpoint hash.
    pub checkpoint_hash: String,
    /// Provider-advertised short `CommitLLM` pin.
    pub commitllm_pin: String,
    /// Hash of the verifier key advertised by the provider.
    pub key_hash: String,
}

/// Result of the optional `/healthz` preflight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthPreflight {
    /// Provider metadata fetched from `/healthz`.
    pub health: ProviderHealth,
    /// Non-fatal warnings to include in eventual JSON output.
    pub warnings: Vec<String>,
    /// WARN log line to write when warnings are present.
    pub warn_line: Option<String>,
}

/// JSON request body for `POST /v1/audit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRequest {
    /// Hash of the `VIRC` receipt being challenged.
    pub receipt_hash: String,
    /// Verifier-selected audit challenge.
    pub challenge: AuditChallenge,
}

impl AuditRequest {
    /// Construct an audit request from a receipt hash and verifier challenge.
    #[must_use]
    pub fn new(receipt_hash: impl Into<String>, challenge: AuditChallenge) -> Self {
        Self {
            receipt_hash: receipt_hash.into(),
            challenge,
        }
    }

    fn to_json_body(&self) -> String {
        serde_json::json!({
            "receipt_hash": &self.receipt_hash,
            "tier": self.challenge.tier.as_str(),
            "challenge": {
                "token_index": self.challenge.token_index,
                "layer_indices": &self.challenge.layer_indices,
            },
        })
        .to_string()
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

/// Raw `POST /v1/audit` response containing a `VIAU` envelope body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditResponse {
    /// HTTP status code.
    pub status: u16,
    /// Endpoint URL that produced this response.
    pub endpoint: String,
    /// Response content type, when present.
    pub content_type: Option<String>,
    /// Raw `VIAU` envelope bytes.
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

/// Parse a plain OpenAI-compatible chat response where no receipt was requested.
pub fn parse_openai_chat_response(response: &ChatResponse) -> Result<String, ViError> {
    chat_text_from_json(&response.body)
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

fn parse_provider_health(bytes: &[u8]) -> Result<ProviderHealth, ViError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| corrupt_healthz("healthz JSON is malformed"))?;

    Ok(ProviderHealth {
        status: required_string(&value, "status")?,
        model_id: required_string(&value, "model_id")?,
        checkpoint_hash: required_string(&value, "checkpoint_hash")?,
        commitllm_pin: required_string(&value, "commitllm_pin")?,
        key_hash: required_string(&value, "key_hash")?,
    })
}

fn required_string(value: &Value, field: &'static str) -> Result<String, ViError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| corrupt_healthz(missing_healthz_field_reason(field)))
}

fn missing_healthz_field_reason(field: &str) -> &'static str {
    match field {
        "status" => "healthz JSON missing status",
        "model_id" => "healthz JSON missing model_id",
        "checkpoint_hash" => "healthz JSON missing checkpoint_hash",
        "commitllm_pin" => "healthz JSON missing commitllm_pin",
        "key_hash" => "healthz JSON missing key_hash",
        _ => "healthz JSON missing required field",
    }
}

fn compare_commitllm_pin(
    health: ProviderHealth,
    expected_short_pin: &str,
    endpoint: &str,
) -> HealthPreflight {
    if health.commitllm_pin == expected_short_pin {
        return HealthPreflight {
            health,
            warnings: Vec::new(),
            warn_line: None,
        };
    }

    let warning = format!(
        "commitllm_pin_mismatch: expected {expected_short_pin}, actual {}",
        health.commitllm_pin
    );
    let warn_line = format!(
        "WARN provider_commitllm_pin_mismatch endpoint={endpoint} expected={expected_short_pin} actual={}",
        health.commitllm_pin
    );

    HealthPreflight {
        health,
        warnings: vec![warning],
        warn_line: Some(warn_line),
    }
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

fn corrupt_healthz(reason: &'static str) -> ViError {
    ViError::CorruptEnvelope {
        envelope: "healthz",
        offset: 0,
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

fn http_status_error(endpoint: &str, status: u16) -> ViError {
    ViError::Network {
        endpoint: endpoint.to_owned(),
        kind: NetworkErrorKind::HttpStatus,
        http_status: Some(status),
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
        time::Duration,
    };

    use mockito::Matcher;
    use reqwest::{header::AUTHORIZATION, Url};
    use serde_json::Value;
    use vi_errors::ViError;
    use vi_receipt::{AuditChallenge, AuditTier};

    use super::{
        parse_chat_response, parse_openai_chat_response, AuditRequest, ChatClient, ChatResponse,
        API_KEY_ENV, RECEIPT_HEADER, TRACE_HEADER,
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
        let headers = client
            .chat_headers_with_receipt("trace-123", true)
            .expect("headers build");

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
    fn mockito_happy_chat_returns_text_and_receipt() {
        let mut server = mockito::Server::new();
        let body = multipart_body(&[
            (
                "application/json",
                br#"{"choices":[{"message":{"content":"hello mock"}}]}"#.as_slice(),
            ),
            (
                "application/vnd.verifiable-intelligence.receipt+binary",
                b"VIRC-mock-receipt".as_slice(),
            ),
        ]);
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("content-type", "application/json")
            .match_header("x-verifiable-receipt", "1")
            .match_header("x-verifiable-intelligence-trace", "trace-chat")
            .match_header("authorization", "Bearer secret-token")
            .match_body(Matcher::Json(serde_json::json!({"messages": []})))
            .with_status(200)
            .with_header("content-type", "multipart/mixed; boundary=test-boundary")
            .with_body(body)
            .create();
        let client = http_test_client(&server.url(), Some("secret-token".to_owned()));

        let response = client
            .post_chat_completions("trace-chat", r#"{"messages":[]}"#)
            .expect("chat request should succeed");
        let parsed = parse_chat_response(&response).expect("multipart response parses");
        let headers = client
            .chat_headers_with_receipt("trace-chat", true)
            .expect("headers build");

        mock.assert();
        assert_eq!(parsed.text, "hello mock");
        assert_eq!(
            parsed.receipt_bytes.as_deref(),
            Some(b"VIRC-mock-receipt".as_slice())
        );
        assert!(!format!("{headers:?}").contains("secret-token"));
    }

    #[test]
    fn mockito_no_receipt_chat_omits_receipt_header_and_parses_plain_json() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("content-type", "application/json")
            .match_header("x-verifiable-receipt", Matcher::Missing)
            .match_header("x-verifiable-intelligence-trace", "trace-chat")
            .match_body(Matcher::Json(serde_json::json!({
                "messages": [{"role": "user", "content": "hello"}],
                "max_tokens": 32,
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(br#"{"choices":[{"message":{"content":"plain mock"}}]}"#)
            .create();
        let client = http_test_client(&server.url(), None);

        let response = client
            .post_chat_completions_without_receipt(
                "trace-chat",
                serde_json::json!({
                    "messages": [{"role": "user", "content": "hello"}],
                    "max_tokens": 32,
                })
                .to_string(),
            )
            .expect("chat request should succeed");
        let text = parse_openai_chat_response(&response).expect("plain JSON should parse");

        mock.assert();
        assert_eq!(text, "plain mock");
    }

    #[test]
    fn mockito_happy_audit_returns_viau_bytes() {
        let mut server = mockito::Server::new();
        let audit_bytes = b"VIAU\x01\x00mock-audit".to_vec();
        let mock = server
            .mock("POST", "/v1/audit")
            .match_header("content-type", "application/json")
            .match_header("x-verifiable-intelligence-trace", "trace-audit")
            .match_body(Matcher::Json(serde_json::json!({
                "receipt_hash": "sha256:receipt",
                "tier": "routine",
                "challenge": {
                    "token_index": 7,
                    "layer_indices": [0, 2, 4],
                },
            })))
            .with_status(200)
            .with_header(
                "content-type",
                "application/vnd.verifiable-intelligence.audit+binary",
            )
            .with_body(audit_bytes.clone())
            .create();
        let client = http_test_client(&server.url(), None);
        let request = AuditRequest::new(
            "sha256:receipt",
            AuditChallenge::new(AuditTier::Routine, 7, vec![0, 2, 4]),
        );

        let response = client
            .post_audit("trace-audit", &request)
            .expect("audit request should succeed");

        mock.assert();
        assert_eq!(response.body, audit_bytes);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/vnd.verifiable-intelligence.audit+binary")
        );
    }

    #[test]
    fn mockito_chat_missing_receipt_is_receipt_missing() {
        let mut server = mockito::Server::new();
        let body = multipart_body(&[(
            "application/json",
            br#"{"choices":[{"message":{"content":"hello"}}]}"#.as_slice(),
        )]);
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("x-verifiable-receipt", "1")
            .with_status(200)
            .with_header("content-type", "multipart/mixed; boundary=test-boundary")
            .with_body(body)
            .create();
        let client = http_test_client(&server.url(), None);

        let response = client
            .post_chat_completions("trace-chat", "{}")
            .expect("chat request should succeed");
        let error = parse_chat_response(&response).expect_err("receipt should be required");

        mock.assert();
        assert_eq!(
            error,
            ViError::ReceiptMissing {
                endpoint: format!("{}/v1/chat/completions", server.url()),
                content_type: "multipart/mixed; boundary=test-boundary".to_owned(),
            }
        );
    }

    #[test]
    fn mockito_degraded_prover_warning_returns_text_without_receipt() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("x-verifiable-receipt", "1")
            .with_status(503)
            .with_header("content-type", "application/json")
            .with_header("warning", r#"199 - "Receipt unavailable""#)
            .with_body(br#"{"choices":[{"message":{"content":"degraded"}}]}"#)
            .create();
        let client = http_test_client(&server.url(), None);

        let response = client
            .post_chat_completions("trace-chat", "{}")
            .expect("warning response should not be a hard network error");
        let parsed = parse_chat_response(&response).expect("warning JSON parses");

        mock.assert();
        assert_eq!(response.status, 503);
        assert_eq!(parsed.text, "degraded");
        assert_eq!(parsed.receipt_bytes, None);
        assert_eq!(
            parsed.warning.as_deref(),
            Some(r#"199 - "Receipt unavailable""#)
        );
    }

    #[test]
    fn mockito_plain_http_server_rejects_https_tls() {
        let server = mockito::Server::new();
        let endpoint = server.url().replacen("http://", "https://", 1);
        let client = ChatClient::new(&endpoint, None).expect("https URL should be accepted");

        let error = client
            .post_chat_completions("trace-chat", "{}")
            .expect_err("plain HTTP mock server should fail TLS");

        assert_eq!(error.category(), "network");
    }

    #[test]
    fn audit_endpoint_round_trips_with_mock_server() {
        let audit_body = b"VIAU\x01\x00audit-envelope".to_vec();
        let (endpoint, server) = serve_http_once(
            "HTTP/1.1 200 OK",
            &[(
                "Content-Type",
                "application/vnd.verifiable-intelligence.audit+binary",
            )],
            audit_body.clone(),
        );
        let client = http_test_client(&endpoint, Some("secret-token".to_owned()));
        let request = AuditRequest::new(
            "sha256:receipt",
            AuditChallenge::new(AuditTier::Deep, 12, vec![1, 7, 13]),
        );

        let response = client
            .post_audit("trace-audit", &request)
            .expect("audit request should succeed");
        let captured = server.join().expect("server joins");

        assert_eq!(response.status, 200);
        assert_eq!(response.endpoint, format!("{endpoint}/v1/audit"));
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/vnd.verifiable-intelligence.audit+binary")
        );
        assert_eq!(response.body, audit_body);
        assert!(captured.starts_with("POST /v1/audit HTTP/1.1\r\n"));
        assert!(captured.contains("x-verifiable-intelligence-trace: trace-audit\r\n"));
        assert!(captured.contains("authorization: Bearer secret-token\r\n"));
        assert_eq!(
            request_json(&captured),
            serde_json::json!({
                "receipt_hash": "sha256:receipt",
                "tier": "deep",
                "challenge": {
                    "token_index": 12,
                    "layer_indices": [1, 7, 13],
                },
            })
        );
    }

    #[test]
    fn audit_4xx_is_network_error_with_http_status() {
        let (endpoint, server) = serve_http_once(
            "HTTP/1.1 400 Bad Request",
            &[("Content-Type", "application/json")],
            br#"{"error":true}"#.to_vec(),
        );
        let client = http_test_client(&endpoint, None);
        let request = AuditRequest::new(
            "sha256:receipt",
            AuditChallenge::new(AuditTier::Routine, 3, vec![0, 1]),
        );

        let error = client
            .post_audit("trace-audit", &request)
            .expect_err("4xx should fail");
        server.join().expect("server joins");

        assert_eq!(
            error,
            ViError::Network {
                endpoint: format!("{endpoint}/v1/audit"),
                kind: vi_errors::NetworkErrorKind::HttpStatus,
                http_status: Some(400),
            }
        );
    }

    #[test]
    fn audit_5xx_is_network_error_with_http_status() {
        let (endpoint, server) = serve_http_once(
            "HTTP/1.1 503 Service Unavailable",
            &[("Content-Type", "application/json")],
            br#"{"error":true}"#.to_vec(),
        );
        let client = http_test_client(&endpoint, None);
        let request = AuditRequest::new(
            "sha256:receipt",
            AuditChallenge::new(AuditTier::Routine, 3, vec![0, 1]),
        );

        let error = client
            .post_audit("trace-audit", &request)
            .expect_err("5xx should fail");
        server.join().expect("server joins");

        assert_eq!(
            error,
            ViError::Network {
                endpoint: format!("{endpoint}/v1/audit"),
                kind: vi_errors::NetworkErrorKind::HttpStatus,
                http_status: Some(503),
            }
        );
    }

    #[test]
    fn healthz_preflight_pin_match_is_silent() {
        let (endpoint, server) = serve_http_once(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "application/json")],
            healthz_body("25541e83"),
        );
        let client = http_test_client(&endpoint, None);

        let preflight = client
            .preflight_commitllm_pin("25541e83")
            .expect("preflight should succeed");
        let captured = server.join().expect("server joins");

        assert_eq!(preflight.health.commitllm_pin, "25541e83");
        assert!(preflight.warnings.is_empty());
        assert_eq!(preflight.warn_line, None);
        assert!(captured.starts_with("GET /healthz HTTP/1.1\r\n"));
    }

    #[test]
    fn healthz_preflight_pin_mismatch_warns_without_failing() {
        let (endpoint, server) = serve_http_once(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "application/json")],
            healthz_body("deadbeef"),
        );
        let client = http_test_client(&endpoint, Some("secret-token".to_owned()));

        let preflight = client
            .preflight_commitllm_pin("25541e83")
            .expect("mismatch should warn but not fail");
        let captured = server.join().expect("server joins");

        assert_eq!(
            preflight.warnings,
            vec!["commitllm_pin_mismatch: expected 25541e83, actual deadbeef"]
        );
        let expected_warn_line =
            format!("WARN provider_commitllm_pin_mismatch endpoint={endpoint}/healthz expected=25541e83 actual=deadbeef");
        assert_eq!(
            preflight.warn_line.as_deref(),
            Some(expected_warn_line.as_str())
        );
        assert!(captured.starts_with("GET /healthz HTTP/1.1\r\n"));
        assert!(captured.contains("authorization: Bearer secret-token\r\n"));
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
        let body = multipart_body(parts);

        ChatResponse {
            status: 200,
            endpoint: "https://provider.example/v1/chat/completions".to_owned(),
            content_type: Some("multipart/mixed; boundary=test-boundary".to_owned()),
            warning: None,
            body,
        }
    }

    fn multipart_body(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (content_type, part_body) in parts {
            body.extend_from_slice(b"--test-boundary\r\nContent-Type: ");
            body.extend_from_slice(content_type.as_bytes());
            body.extend_from_slice(b"\r\n\r\n");
            body.extend_from_slice(part_body);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--test-boundary--\r\n");
        body
    }

    fn healthz_body(commitllm_pin: &str) -> Vec<u8> {
        serde_json::json!({
            "status": "ok",
            "model_id": "llama-3.1-8b-w8a8",
            "checkpoint_hash": "sha256:checkpoint",
            "commitllm_pin": commitllm_pin,
            "key_hash": "sha256:key",
        })
        .to_string()
        .into_bytes()
    }

    fn http_test_client(endpoint: &str, api_key: Option<String>) -> ChatClient {
        ChatClient::new_unchecked(Url::parse(endpoint).expect("test URL parses"), api_key)
    }

    fn serve_http_once(
        status_line: &'static str,
        headers: &'static [(&'static str, &'static str)],
        body: Vec<u8>,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener binds");
        let endpoint = format!("http://{}", listener.local_addr().expect("listener addr"));
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("connection accepted");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout set");
            let request = read_http_request(&mut stream);

            let mut response = format!("{status_line}\r\nContent-Length: {}\r\n", body.len());
            for (name, value) in headers {
                response.push_str(name);
                response.push_str(": ");
                response.push_str(value);
                response.push_str("\r\n");
            }
            response.push_str("Connection: close\r\n\r\n");
            stream
                .write_all(response.as_bytes())
                .expect("response headers written");
            stream.write_all(&body).expect("response body written");
            request
        });
        (endpoint, handle)
    }

    fn read_http_request(stream: &mut impl Read) -> String {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).expect("request bytes read");
            assert!(read != 0, "connection closed before full request");
            buffer.extend_from_slice(&chunk[..read]);

            let Some(header_end) = find_header_end(&buffer) else {
                continue;
            };
            let body_len = content_length(&buffer[..header_end]);
            if buffer.len() >= header_end + 4 + body_len {
                return String::from_utf8(buffer).expect("request is UTF-8");
            }
        }
    }

    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes
            .windows(b"\r\n\r\n".len())
            .position(|window| window == b"\r\n\r\n")
    }

    fn content_length(headers: &[u8]) -> usize {
        let headers = std::str::from_utf8(headers).expect("headers are UTF-8");
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("content length parses")
                })
            })
            .unwrap_or(0)
    }

    fn request_json(request: &str) -> Value {
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("request contains body separator");
        serde_json::from_str(body).expect("request body is JSON")
    }
}
