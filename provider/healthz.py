#!/usr/bin/env python3
"""Tiny provider health server used by the provider entrypoint."""

from __future__ import annotations

import argparse
import json
import os
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


STARTED_AT = time.monotonic()


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
    server_version = "vi-healthz/0"

    def do_GET(self) -> None:
        if self.path != "/healthz":
            self.send_json(HTTPStatus.NOT_FOUND, {"error": "not found"})
            return
        self.send_json(HTTPStatus.OK, metadata())

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def send_json(self, status: HTTPStatus, payload: dict[str, object]) -> None:
        body = json.dumps(payload, separators=(",", ":"), sort_keys=True).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("cache-control", "no-store")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


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
