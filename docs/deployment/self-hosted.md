# Self-hosted Provider

This guide runs the provider container with Docker Compose. The default service
uses the CPU stub path so `/healthz`, provider lifecycle logs, and provider API
guards can be tested without a GPU or live weights.

Use this path when you want the most debuggable local deployment. For the
managed endpoint path, see [Hugging Face deployment](./hf.md).

## Prerequisites

- Docker Engine with the Compose V2 plugin.
- `curl` for smoke tests.
- Access to this repository root.
- For the GPU profile only: NVIDIA driver, NVIDIA Container Toolkit, one GPU
  with enough VRAM for the selected W8A8 model, canonical checkpoint hash, and
  verifier key hash.

The default CPU stub does not require a GPU or model weights. It is a deployment
smoke, not a live CommitLLM receipt path.

## CPU Stub

From the repository root:

```bash
docker compose -f provider/compose.yaml up --build provider
```

Example ready log:

```json
{"commitllm_pin":"25541e83","event":"provider.ready","healthz":"http://127.0.0.1:8000/healthz","level":"info","port":8000}
```

Healthcheck:

```bash
docker compose -f provider/compose.yaml ps
curl -fsS http://127.0.0.1:8000/healthz | python3 -m json.tool
```

Chat smoke:

```bash
curl -fsS http://127.0.0.1:8000/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"hello"}],"max_tokens":4096}' \
  | python3 -m json.tool
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

Healthcheck:

```bash
docker compose -f provider/compose.yaml --profile gpu ps
curl -fsS http://127.0.0.1:8000/healthz | python3 -m json.tool
```

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

## Logs

Follow provider lifecycle logs with:

```bash
docker compose -f provider/compose.yaml logs -f provider
```

Expected events include `provider.boot`, `provider.ready`, and
`provider.shutdown`. The boot and health metadata must agree on
`commitllm_pin`, `model_id`, `checkpoint_hash`, and `key_hash`.

## Shutdown

Stop the provider with:

```bash
docker compose -f provider/compose.yaml down
```

The entrypoint handles `SIGTERM`, emits `provider.shutdown`, and exits cleanly.

## Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `docker compose` is missing | Old Docker install or Compose V1 only | Install Docker Compose V2 and rerun `docker compose version`. |
| `Cannot connect to the Docker daemon` | Docker Engine is not running | Start Docker Desktop or the system Docker service. |
| Port `8000` is already allocated | Another local service is bound | Set `VI_PROVIDER_PORT=8001` and use `http://127.0.0.1:8001`. |
| `/healthz` shows zero hashes | Stub/default metadata | Valid for deployment smoke only; set real `VI_CHECKPOINT_HASH` and `VI_KEY_HASH` before live claims. |
| Chat requests return `429` | Per-IP rate limit tripped | Lower request rate or raise `VI_RATE_LIMIT_RPM` for local testing. |
| Chat request returns `413` | Body exceeds `VI_CHAT_BODY_LIMIT_BYTES` | Reduce prompt size or raise the body limit for local testing. |
| GPU profile cannot see a GPU | NVIDIA runtime not configured | Install NVIDIA Container Toolkit and verify `docker run --gpus all nvidia/cuda:12.4.1-runtime-ubuntu22.04 nvidia-smi`. |
| GPU profile exits with `vllm: not found` | The current image is still a provider skeleton | Use the CPU stub until the live vLLM provider image gate lands. |
| `vi chat` reports `receipt_missing` | Stub path returns JSON only | Use `vi chat --no-receipt` for stub testing; use the live GPU path for receipt tests once unblocked. |
