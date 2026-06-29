#!/usr/bin/env python3
"""Tiny provider stub server used by the provider entrypoint."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import time
from collections import defaultdict, deque
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


STARTED_AT = time.monotonic()
SCHEMA_VERSION = 1
RATE_LIMIT_WINDOW_S = float(os.environ.get("VI_RATE_LIMIT_WINDOW_S", "300"))
RATE_BUCKETS: dict[str, dict[str, deque[float]]] = {
    "chat": defaultdict(deque),
    "audit": defaultdict(deque),
}


def metadata() -> dict[str, object]:
    return {
        "status": "ok",
        "model_id": os.environ.get("MODEL_ID", "llama-3.1-8b-w8a8"),
        "checkpoint_hash": os.environ.get("VI_CHECKPOINT_HASH", zero_hash()),
        "commitllm_pin": os.environ.get("COMMITLLM_SHORT", "25541e83"),
        "key_hash": os.environ.get("VI_KEY_HASH", zero_hash()),
        "uptime_s": int(time.monotonic() - STARTED_AT),
    }


def zero_hash() -> str:
    return "sha256:" + ("0" * 64)


class Handler(BaseHTTPRequestHandler):
    server_version = "vi-provider-stub/0"

    def do_GET(self) -> None:
        if self.path != "/healthz":
            self.send_json(HTTPStatus.NOT_FOUND, {"error": "not found"})
            return
        self.send_json(HTTPStatus.OK, metadata())

    def do_POST(self) -> None:
        if self.path == "/v1/chat/completions":
            self.handle_chat()
            return
        if self.path == "/v1/audit":
            self.handle_audit()
            return
        self.send_json(HTTPStatus.NOT_FOUND, {"error": "not found"})

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def handle_chat(self) -> None:
        allowed, retry_after = check_rate_limit(
            "chat", self.client_ip(), rate_limit("VI_RATE_LIMIT_RPM", 12)
        )
        if not allowed:
            self.send_error_envelope(
                HTTPStatus.TOO_MANY_REQUESTS,
                "rate_limit",
                "chat rate limit exceeded",
                {"retry_after_s": retry_after},
            )
            return

        body = self.read_limited_json(chat_body_limit())
        if body is None:
            return
        requested = body.get("max_tokens", max_tokens_limit())
        if not isinstance(requested, int):
            self.send_error_envelope(
                HTTPStatus.BAD_REQUEST,
                "input",
                "max_tokens must be an integer",
                {"field": "max_tokens"},
            )
            return

        effective = min(max(requested, 1), max_tokens_limit())
        self.send_json(
            HTTPStatus.OK,
            {
                "id": "chatcmpl-provider-stub",
                "object": "chat.completion",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "provider stub response",
                        },
                        "finish_reason": "stop",
                    }
                ],
                "verifiable_intelligence": {
                    "max_tokens_requested": requested,
                    "max_tokens_effective": effective,
                    "max_tokens_clamped": effective != requested,
                },
            },
        )

    def handle_audit(self) -> None:
        allowed, retry_after = check_rate_limit(
            "audit", self.client_ip(), rate_limit("VI_AUDIT_RATE_LIMIT_RPM", 120)
        )
        if not allowed:
            self.send_error_envelope(
                HTTPStatus.TOO_MANY_REQUESTS,
                "rate_limit",
                "audit rate limit exceeded",
                {"retry_after_s": retry_after},
            )
            return

        body = self.read_limited_json(audit_body_limit())
        if body is None:
            return
        request_id = "aud_" + hashlib.sha256(
            json.dumps(body, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()[:8]
        payload = json.dumps(
            {
                "request_id": request_id,
                "receipt_hash": body.get("receipt_hash", ""),
                "tier": body.get("tier", ""),
                "challenge": body.get("challenge", {}),
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
        self.send_bytes(
            HTTPStatus.OK,
            "application/vnd.verifiable-intelligence.audit+binary",
            payload,
        )

    def read_limited_json(self, limit: int) -> dict[str, object] | None:
        length_header = self.headers.get("content-length")
        try:
            length = int(length_header or "0")
        except ValueError:
            self.send_error_envelope(
                HTTPStatus.BAD_REQUEST,
                "input",
                "content-length must be an integer",
                {"field": "content-length"},
            )
            return None
        if length > limit:
            self.send_error_envelope(
                HTTPStatus.REQUEST_ENTITY_TOO_LARGE,
                "input",
                "request body too large",
                {"limit_bytes": limit, "actual_bytes": length},
            )
            return None
        body = self.rfile.read(length)
        try:
            parsed = json.loads(body or b"{}")
        except json.JSONDecodeError:
            self.send_error_envelope(
                HTTPStatus.BAD_REQUEST,
                "input",
                "request body must be valid JSON",
                {},
            )
            return None
        if not isinstance(parsed, dict):
            self.send_error_envelope(
                HTTPStatus.BAD_REQUEST,
                "input",
                "request body must be a JSON object",
                {},
            )
            return None
        return parsed

    def client_ip(self) -> str:
        forwarded = self.headers.get("x-forwarded-for")
        if forwarded:
            return forwarded.split(",", 1)[0].strip()
        return self.client_address[0]

    def send_json(self, status: HTTPStatus, payload: dict[str, object]) -> None:
        body = json.dumps(payload, separators=(",", ":"), sort_keys=True).encode()
        self.send_bytes(status, "application/json; charset=utf-8", body)

    def send_bytes(self, status: HTTPStatus, content_type: str, body: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("cache-control", "no-store")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_error_envelope(
        self,
        status: HTTPStatus,
        category: str,
        message: str,
        detail: dict[str, object],
    ) -> None:
        self.send_json(
            status,
            {
                "error": True,
                "schema_version": SCHEMA_VERSION,
                "subcommand": "provider",
                "category": category,
                "exit_code": int(status),
                "message": message,
                "detail": detail,
                "trace_id": self.headers.get("x-verifiable-intelligence-trace", ""),
            },
        )


def rate_limit(env_name: str, default_per_minute: int) -> int:
    per_minute = int(os.environ.get(env_name, str(default_per_minute)))
    return max(1, int(per_minute * RATE_LIMIT_WINDOW_S / 60))


def check_rate_limit(bucket: str, ip: str, limit: int) -> tuple[bool, int]:
    now = time.monotonic()
    entries = RATE_BUCKETS[bucket][ip]
    while entries and now - entries[0] >= RATE_LIMIT_WINDOW_S:
        entries.popleft()
    if len(entries) >= limit:
        retry_after = max(1, int(RATE_LIMIT_WINDOW_S - (now - entries[0])))
        return False, retry_after
    entries.append(now)
    return True, 0


def chat_body_limit() -> int:
    return int(os.environ.get("VI_CHAT_BODY_LIMIT_BYTES", "32768"))


def audit_body_limit() -> int:
    return int(os.environ.get("VI_AUDIT_BODY_LIMIT_BYTES", "32768"))


def max_tokens_limit() -> int:
    return int(os.environ.get("VI_MAX_TOKENS", "1024"))


def parse_bind(value: str) -> tuple[str, int]:
    if ":" not in value:
        raise SystemExit(f"VI_BIND_ADDR must be host:port, got {value!r}")
    host, port = value.rsplit(":", 1)
    try:
        parsed_port = int(port)
    except ValueError as error:
        raise SystemExit(f"VI_BIND_ADDR port must be an integer, got {port!r}") from error
    return host, parsed_port


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bind", default=os.environ.get("VI_BIND_ADDR", "0.0.0.0:8000"))
    args = parser.parse_args()
    host, port = parse_bind(args.bind)
    ThreadingHTTPServer((host, port), Handler).serve_forever()


if __name__ == "__main__":
    main()
