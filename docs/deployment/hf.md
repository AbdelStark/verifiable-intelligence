# Hugging Face Endpoint Deployment

This guide deploys the provider container to Hugging Face Inference Endpoints
as a custom container. It is the managed deployment path for contributors who
want a public URL without operating a GPU host.

The currently runnable path is the CPU stub. It validates the image,
entrypoint, `/healthz`, OpenAI-compatible chat route, rate limits, and metadata
surface on Hugging Face. It does not produce a real CommitLLM receipt. The live
GPU path is documented below, but do not advertise it as a proof-producing
deployment until the canonical checkpoint hash, verifier key, and live provider
image gates are complete.

Relevant Hugging Face docs:

- Custom containers: <https://huggingface.co/docs/inference-endpoints/en/guides/custom_container>
- `hf endpoints` CLI: <https://huggingface.co/docs/huggingface_hub/en/guides/cli#hf-endpoints>
- Pricing: <https://huggingface.co/docs/inference-endpoints/pricing>

## Prerequisites

- Hugging Face account with Inference Endpoints access.
- Active billing for any paid endpoint. Hugging Face bills deployed endpoints
  while they are initializing or running, with cost calculated by the minute.
- `hf` CLI authenticated with an access token:

  ```bash
  curl -LsSf https://hf.co/cli/install.sh | bash
  hf auth login
  ```

- Docker with Buildx and permission to push to a registry Hugging Face can
  pull from, such as GHCR, Docker Hub, ECR, ACR, or GCR.
- `curl`, `python3`, and a local Rust toolchain if you want to run `vi chat`.
- For the live GPU path only: GPU quota in the selected Hugging Face region,
  the canonical checkpoint hash, the verifier key hash, and an image that
  includes the live vLLM plus CommitLLM serving path.

Set the common environment:

```bash
export HF_TOKEN="hf_..."
export HF_ENDPOINT_NAME="vi-provider-stub"
export HF_REGION="us-east-1"
export HF_VENDOR="aws"
export VI_PROVIDER_IMAGE="ghcr.io/<owner>/verifiable-intelligence-provider:$(git rev-parse --short HEAD)"
```

If you push to GHCR, authenticate Docker before building:

```bash
printf '%s' "$GHCR_TOKEN" | docker login ghcr.io -u <github-user> --password-stdin
```

## Build and Push

From the repository root:

```bash
docker buildx build \
  --platform linux/amd64 \
  -f provider/Dockerfile \
  -t "$VI_PROVIDER_IMAGE" \
  --push \
  .
```

Example output:

```text
#31 exporting manifest list sha256:71b0...
#31 pushing layers
#31 pushing manifest for ghcr.io/<owner>/verifiable-intelligence-provider:fc622b2
```

Pin the deployment to the pushed image digest:

```bash
docker buildx imagetools inspect "$VI_PROVIDER_IMAGE" \
  --format '{{.Manifest.Digest}}'

export VI_PROVIDER_IMAGE_PINNED="ghcr.io/<owner>/verifiable-intelligence-provider@sha256:<digest>"
```

Use the digest-pinned reference for all endpoint creation. A tag alone is
mutable and makes proof metadata harder to audit later.

## Deploy CPU Stub

The stub endpoint is the safe first Hugging Face deployment. It should become
healthy without model weights or a GPU.

```bash
hf endpoints deploy "$HF_ENDPOINT_NAME" \
  --repo gpt2 \
  --framework custom \
  --accelerator cpu \
  --vendor "$HF_VENDOR" \
  --region "$HF_REGION" \
  --instance-type intel-spr \
  --instance-size x2 \
  --custom-image "$VI_PROVIDER_IMAGE_PINNED" \
  --health-route /healthz \
  --port 8000 \
  --env VI_PROVIDER_STUB=1 \
  --env MODEL_ID=llama-3.1-8b-w8a8 \
  --env COMMITLLM_SHORT=25541e83 \
  --env VI_CHECKPOINT_HASH=sha256:0000000000000000000000000000000000000000000000000000000000000000 \
  --env VI_KEY_HASH=sha256:0000000000000000000000000000000000000000000000000000000000000000 \
  --type authenticated
```

Example output:

```text
Endpoint vi-provider-stub created
Status: pending
```

Wait for `status: running` and copy the endpoint URL:

```bash
hf endpoints describe "$HF_ENDPOINT_NAME"
```

Example output:

```text
name: vi-provider-stub
status: running
url: https://<endpoint-id>.<region>.<vendor>.endpoints.huggingface.cloud
```

Set the URL for smoke tests:

```bash
export HF_ENDPOINT_URL="https://<endpoint-id>.<region>.<vendor>.endpoints.huggingface.cloud"
```

## Smoke Test

Check readiness and metadata:

```bash
curl -fsS "$HF_ENDPOINT_URL/healthz" \
  -H "Authorization: Bearer $HF_TOKEN" \
  | python3 -m json.tool
```

Expected shape:

```json
{
  "checkpoint_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "commitllm_pin": "25541e83",
  "key_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "model_id": "llama-3.1-8b-w8a8",
  "status": "ok",
  "uptime_s": 12
}
```

Check the OpenAI-compatible chat route:

```bash
curl -fsS "$HF_ENDPOINT_URL/v1/chat/completions" \
  -H "Authorization: Bearer $HF_TOKEN" \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"hello"}],"max_tokens":4096}' \
  | python3 -m json.tool
```

The stub response should include
`verifiable_intelligence.max_tokens_effective: 1024`, proving the request guard
is active.

Optional CLI smoke:

```bash
cargo run -p verifiable-intelligence -- \
  chat \
  --endpoint "$HF_ENDPOINT_URL" \
  --api-key "$HF_TOKEN" \
  --prompt "hello from hf" \
  --no-receipt \
  --pretty
```

Use `--no-receipt` for the stub. A receipt-required chat against the stub is
expected to fail with `receipt_missing`.

## Live GPU Switch

Use this path only after the live provider gates have landed:

- `provider/Dockerfile` contains the real vLLM plus CommitLLM runtime.
- The resolved W8A8 model revision has a canonical checkpoint hash.
- The verifier key and key hash have been generated at CommitLLM pin
  `25541e83`.
- The endpoint has enough GPU memory for the selected model.

Deploy the same image as a GPU endpoint:

```bash
export HF_ENDPOINT_NAME="vi-provider-live"
export VI_CHECKPOINT_HASH="sha256:<canonical-checkpoint-hash>"
export VI_KEY_HASH="sha256:<verifier-key-hash>"

hf endpoints deploy "$HF_ENDPOINT_NAME" \
  --repo RedHatAI/Meta-Llama-3.1-8B-Instruct-quantized.w8a8 \
  --framework custom \
  --accelerator gpu \
  --vendor "$HF_VENDOR" \
  --region "$HF_REGION" \
  --instance-type nvidia-a100 \
  --instance-size x1 \
  --custom-image "$VI_PROVIDER_IMAGE_PINNED" \
  --health-route /healthz \
  --port 8000 \
  --env VI_PROVIDER_STUB=0 \
  --env MODEL_DIR=/repository \
  --env MODEL_ID=llama-3.1-8b-w8a8 \
  --env COMMITLLM_SHORT=25541e83 \
  --env VI_CHECKPOINT_HASH="$VI_CHECKPOINT_HASH" \
  --env VI_KEY_HASH="$VI_KEY_HASH" \
  --type authenticated
```

Once it is running, the live proof smoke is:

```bash
export HF_ENDPOINT_URL="https://<live-endpoint-id>.<region>.<vendor>.endpoints.huggingface.cloud"

cargo run -p verifiable-intelligence -- \
  chat \
  --endpoint "$HF_ENDPOINT_URL" \
  --api-key "$HF_TOKEN" \
  --prompt "prove the deployed model identity" \
  --max-tokens 64 \
  --receipt-out /tmp/vi-hf.virc

cargo run -p verifiable-intelligence -- \
  verify \
  --receipt /tmp/vi-hf.virc \
  --key provider/keys/llama-3.1-8b-w8a8/commitllm-25541e83/verifier-key.viky \
  --audit-endpoint "$HF_ENDPOINT_URL" \
  --api-key "$HF_TOKEN" \
  --pretty
```

Do not use zero hashes in a live proof claim.

## Cost Estimate

Check the Hugging Face pricing page before running. As of 2026-06-29, the page
lists endpoint pricing by hour but says actual cost is calculated by the minute.

Formula:

```text
estimated_cost = hourly_rate * running_minutes / 60
```

Examples:

| Path | Instance | Published hourly rate | 30 minute smoke |
| --- | --- | ---: | ---: |
| CPU stub | `aws intel-spr x2` | `$0.067` | about `$0.04` |
| Small GPU probe | `aws nvidia-l4 x1` | `$0.80` | about `$0.40` |
| Reference live target | `aws nvidia-a100 x1` | `$2.50` | about `$1.25` |

Costs continue until you pause or delete the endpoint.

## Teardown

Pause when you expect to reuse the endpoint:

```bash
hf endpoints pause "$HF_ENDPOINT_NAME"
```

Delete when you are done with the smoke:

```bash
hf endpoints delete "$HF_ENDPOINT_NAME" --yes
```

Confirm the endpoint is gone or paused:

```bash
hf endpoints describe "$HF_ENDPOINT_NAME"
```

## Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `hf: command not found` | CLI not installed or shell not refreshed | Re-run the installer, open a new shell, then run `hf --help`. |
| Endpoint create fails with auth error | Missing or insufficient `HF_TOKEN` | Run `hf auth login` and confirm the token can manage Inference Endpoints. |
| Image pull fails | Private registry credentials or wrong image digest | Use a registry Hugging Face can pull from, verify `docker pull "$VI_PROVIDER_IMAGE_PINNED"` from a clean host, or make the image public for the smoke. |
| Endpoint stays `pending` | Quota or unavailable hardware in region | Try a smaller CPU stub first, request quota, or change `HF_REGION`/instance type. |
| Endpoint stays `initializing` | Container never passes the health route | Confirm `--health-route /healthz`, `--port 8000`, and check endpoint logs. |
| `/healthz` shows zero hashes | Stub or placeholder metadata | Fine for deployment smoke; invalid for live proof claims. Set canonical hashes before live use. |
| `vi chat` returns `receipt_missing` | Stub mode returns JSON only | Use `--no-receipt` for the stub, or deploy the live GPU receipt path after its gates land. |
| Live endpoint logs `vllm: not found` | Image does not yet include the real vLLM runtime | Rebuild after the provider image gate lands; use the CPU stub until then. |
| GPU endpoint is out of memory | Instance too small for the selected W8A8 model | Start with A100 x1 for the reference path, then downshift only after a measured live run. |
