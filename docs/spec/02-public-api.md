# 02 — Public API

This document specifies the public surfaces a downstream consumer can rely on across v1: the `vi` CLI, the provider HTTP API (extensions to the OpenAI-compatible chat completion path), and the on-disk artifact contracts. Versioning policy for these surfaces is in [09-release-and-versioning.md](./09-release-and-versioning.md). Detailed CLI shape is in [RFC-0002](../rfcs/RFC-0002-cli-surface.md).

## 1. CLI surface

### 1.1 Binary

- Name: `vi`
- Installation: `cargo install verifiable-intelligence`; prebuilt binaries published for Linux x86_64 and macOS arm64 via GitHub Releases ([FR-7](../../PRD.md)).
- Windows: best-effort, not gated.

### 1.2 Subcommand contract

All subcommands obey these invariants:

- Output is JSON to stdout by default; `--pretty` switches to a human-readable formatted view. Inversion from common CLIs is intentional (PRD's primary persona is CI integration).
- Errors go to stderr in the same JSON envelope; structured error envelope shape in [04-error-model.md](./04-error-model.md).
- Exit codes are stable per [RFC-0014](../rfcs/RFC-0014-error-taxonomy.md).
- Every flag is documented in `--help`; `--help` output is part of the public API and is regression-tested.

Subcommand summary table:

| Subcommand | Purpose | Networked | Exit codes |
|------------|---------|-----------|------------|
| `vi keygen` | Generate verifier key from a model checkpoint | optional (may fetch weights) | 0 success, 2 input, 3 network, 4 hash mismatch |
| `vi chat` | Send a prompt, receive text + receipt | yes | 0 success, 2 input, 3 network, 5 receipt missing |
| `vi verify` | Verify a receipt against a key | optional (audit endpoint) | 0 pass, 1 fail, 2 input, 3 network, 6 unknown-version, 7 unknown-model |
| `vi tui` | Interactive TUI | yes | 0 normal exit, 130 SIGINT |

Detail per subcommand:

#### `vi keygen`

```
vi keygen --model <id> [--checkpoint <path>] [--output <path>] [--seed <u64>]
```

- `--model`: required. Canonical model identifier; v1 supports `llama-3.2-1b-w8a8`. Unknown identifiers fail closed.
- `--checkpoint`: optional. Local path to a checkpoint directory; if omitted, `vi keygen` fetches the published mirror (see [RFC-0012](../rfcs/RFC-0012-w8a8-quantization.md)).
- `--output`: optional, defaults to `./key.bin`. Existing file is not overwritten without `--force`.
- `--seed`: optional, defaults to a documented constant. Determinism is in `(checkpoint_hash, model_id, seed)`.

Output (stdout, JSON):

```json
{
  "subcommand": "keygen",
  "key_path": "./key.bin",
  "key_size_bytes": 8_388_608,
  "model_id": "llama-3.2-1b-w8a8",
  "checkpoint_hash": "sha256:...",
  "seed": 0,
  "key_hash": "sha256:...",
  "schema_version": 1
}
```

`key_hash` is the SHA-256 of the emitted `key.bin`. Two invocations with the same inputs must emit identical bytes ([FR-17](../../PRD.md)).

#### `vi chat`

```
vi chat --endpoint <url> --prompt <text> [--max-tokens <n>] [--receipt-out <path>] [--no-receipt]
```

- `--endpoint`: required. HTTPS URL.
- `--prompt`: required. UTF-8.
- `--receipt-out`: optional, defaults to `./receipt.bin`.
- `--no-receipt`: optional. Disables the receipt opt-in header; the response is plain text. Provided so the CLI is usable as a thin client.

Output (JSON):

```json
{
  "subcommand": "chat",
  "endpoint": "https://...",
  "model_id": "llama-3.2-1b-w8a8",
  "text": "...",
  "receipt_path": "./receipt.bin",
  "receipt_size_bytes": 73_142,
  "elapsed_ms": 1845,
  "schema_version": 1
}
```

If `--no-receipt` was set, `receipt_path` and `receipt_size_bytes` are absent and the schema omits them.

#### `vi verify`

```
vi verify --receipt <path> --key <path> [--tier full|deep|routine|receipt-only] [--audit-endpoint <url>]
```

- `--tier`: optional, defaults to `routine`. `full` requires `--audit-endpoint`.
- `--audit-endpoint`: required if tier is `full` or `deep`. The audit endpoint may be the same host as the chat endpoint or distinct.

Output (JSON), the structured verifier report:

```json
{
  "subcommand": "verify",
  "schema_version": 1,
  "tier": "full",
  "model_id": "llama-3.2-1b-w8a8",
  "key_hash": "sha256:...",
  "receipt_version": 4,
  "phases_checked": [
    "embedding_merkle",
    "shell_freivalds",
    "bridge_replay",
    "attention_corridor",
    "kv_provenance",
    "lm_head",
    "decode_policy"
  ],
  "phases_passed": [...],
  "phases_failed": [
    {
      "phase": "bridge_replay",
      "detail": "L_inf=47, tolerance=10"
    }
  ],
  "overall": "fail",
  "elapsed_ms": 612,
  "warnings": []
}
```

The `phases_*` arrays are stable enumerations of phase identifiers. Adding a phase is a minor-version change; removing one is breaking ([09-release-and-versioning.md](./09-release-and-versioning.md)).

`overall` is `"pass"` only if `phases_failed` is empty AND every phase the requested tier requires is present in `phases_passed`. The CLI never silently elevates a routine audit to a "verified" claim ([FR-9](../../PRD.md)).

#### `vi tui`

```
vi tui --endpoint <url> [--tamper <kind>] [--phase-delay <ms>]
```

- `--tamper`: optional. v1 supports `byte-flip`; other kinds are future work ([FR-11](../../PRD.md)).
- `--phase-delay`: optional, defaults to 0. Inserts deliberate delay between phase transitions for human-readable demonstration ([FR-12](../../PRD.md)).

TUI behaviour spec lives in [RFC-0008](../rfcs/RFC-0008-tui-architecture.md).

### 1.3 Output schema versioning

Every CLI JSON object carries `schema_version: <integer>`. v1 ships `schema_version: 1` on all subcommands. The version field is independent per subcommand. Schema evolution rules in [09-release-and-versioning.md](./09-release-and-versioning.md).

## 2. Provider HTTP surface

The provider exposes an OpenAI-compatible chat-completion API (vLLM upstream) plus extensions.

### 2.1 Chat completion

- Path: `POST /v1/chat/completions`
- Body: OpenAI chat-completion shape.
- Receipt opt-in: clients send `X-Verifiable-Receipt: 1`. When set, the response is `multipart/mixed` with two parts: the chat completion JSON and a binary part of type `application/vnd.verifiable-intelligence.receipt+binary` with the receipt envelope. When unset, the response is the OpenAI shape only. ([RFC-0006](../rfcs/RFC-0006-receipt-api-header.md))
- Streaming: `stream: true` is supported for the text; the receipt is delivered as a trailing multipart part once generation is complete. CLI streaming is post-v1.

### 2.2 Audit endpoint

- Path: `POST /v1/audit`
- Body (JSON):

  ```json
  {
    "request_id": "...",
    "tier": "full|deep|routine",
    "token_index": <integer>,
    "layer_indices": [<integers>]
  }
  ```

- Response: `application/vnd.verifiable-intelligence.audit+binary` with the CommitLLM audit payload bytes per the pinned crate. Magic prefix and version byte are defined in [03-data-model.md](./03-data-model.md).

### 2.3 Health

- Path: `GET /healthz`
- Response: `200 OK` with body `{"status": "ok", "model_id": "...", "checkpoint_hash": "sha256:...", "commitllm_pin": "<commit>"}`.
- Used by HF Endpoints readiness check and by the deploy-recipe smoke test ([RFC-0007](../rfcs/RFC-0007-hf-deployment-recipe.md)).

### 2.4 Versioning

- Path prefix `/v1/` is stable across v1.x.
- Breaking changes go to `/v2/` and are not contemplated for v1.
- The `commitllm_pin` field in `/healthz` is the authoritative pin advertisement; the CLI may use it to detect mismatch and warn.

## 3. On-disk artifact contracts

| Artifact | Format | Versioning | Owner |
|----------|--------|------------|-------|
| Verifier key | Binary, magic `VIKY` + version byte + CommitLLM key blob | Independent semver on the envelope | `vi-keygen` / `vi-receipt` |
| Receipt | Binary, magic `VIRC` + version byte + CommitLLM receipt blob | Tracks CommitLLM receipt schema | `vi-receipt` |
| Audit payload | Binary, magic `VIAU` + version byte + CommitLLM audit blob | Tracks CommitLLM audit schema | `vi-receipt` |

Magic-prefix details, byte layout, and version handshake are normative in [03-data-model.md](./03-data-model.md) and [RFC-0003](../rfcs/RFC-0003-receipt-format-pinning.md).

## 4. Public stability promises

- **Stable across v1.x:** subcommand names, required flag names, JSON field names, exit code map, magic prefixes, HTTP path prefix.
- **Reserved for change:** additional flags (with sensible defaults), additional JSON fields, additional phases reported, additional headers honored.
- **Breaking:** removing or renaming any of the above, changing exit codes, changing magic prefixes.

Breaking changes require a major version bump and a documented deprecation window of at least one minor release ([09-release-and-versioning.md](./09-release-and-versioning.md)).
