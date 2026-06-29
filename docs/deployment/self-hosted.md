# Self-hosted Provider

This guide runs the provider container with Docker Compose. The default service
uses the CPU stub path so `/healthz`, provider lifecycle logs, and provider API
guards can be tested without a GPU or live weights.

## CPU Stub

From the repository root:

```bash
docker compose -f provider/compose.yaml up --build provider
```

Smoke test:

```bash
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8000/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"hello"}],"max_tokens":4096}'
```

The chat response includes `verifiable_intelligence.max_tokens_effective`; it
is clamped to `VI_MAX_TOKENS` and defaults to `1024`.

## GPU Profile

The live GPU path is wired but still depends on the real W8A8 weight artifact
and canonical checkpoint hash tracked by the provider image follow-ups. Once
those are available, run a single-GPU provider with:

```bash
VI_PROVIDER_STUB=0 \
VI_CHECKPOINT_HASH=sha256:<canonical-checkpoint-hash> \
VI_KEY_HASH=sha256:<verifier-key-hash> \
docker compose -f provider/compose.yaml --profile gpu up --build provider-gpu
```

The GPU service reserves one NVIDIA GPU and runs `vllm serve` through
`provider/entrypoint.sh`. Its internal health server listens on
`127.0.0.1:8001` to avoid colliding with vLLM on port `8000`; the Compose
healthcheck probes that internal port.

## Environment

| Variable | Default | Effect |
| --- | --- | --- |
| `VI_PROVIDER_IMAGE` | `verifiable-intelligence-provider:local` | Local image tag used by Compose. |
| `VI_PROVIDER_PORT` | `8000` | Host port mapped to the provider. |
| `VI_PROVIDER_STUB` | `1` | Uses the CPU stub when `1`; launches vLLM when `0`. |
| `VI_MAX_NUM_SEQS` | `8` | vLLM `--max-num-seqs` value. |
| `VI_RATE_LIMIT_RPM` | `12` | Chat request rate per minute, equivalent to 60 per 5 minutes. |
| `VI_AUDIT_RATE_LIMIT_RPM` | `120` | Audit request rate per minute, equivalent to 600 per 5 minutes. |
| `VI_MAX_TOKENS` | `1024` | Maximum accepted generation token request. |
| `VI_LOG_LEVEL` | `info` | Structured provider log level. |
| `MODEL_ID` | `llama-3.1-8b-w8a8` | Provider model identifier advertised by logs and `/healthz`. |
| `MODEL_DIR` | `/weights` | Model directory passed to vLLM. |
| `VI_CHECKPOINT_HASH` | zero hash | Checkpoint hash advertised by `/healthz`. Use the real canonical hash before a live claim. |
| `VI_KEY_HASH` | zero hash | Verifier key hash advertised by `/healthz`. Use the real key hash before a live claim. |

## Shutdown

Stop the provider with:

```bash
docker compose -f provider/compose.yaml down
```

The entrypoint handles `SIGTERM`, emits `provider.shutdown`, and exits cleanly.
