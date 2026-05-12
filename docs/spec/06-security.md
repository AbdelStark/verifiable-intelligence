# 06 — Security

This document states the threat model the project is designed against, the trust boundaries, the assets, the abuser, the secrets handling rules, and the abuse controls on the provider side. The full cryptographic threat model belongs to CommitLLM upstream; this document covers the integration and deployment surface that this project owns.

## Threat model

### Assets we are protecting

1. **The integrity of a single inference response**: the developer can verify that the response came from the advertised model executing the advertised configuration on the advertised input. This is what the CommitLLM protocol provides.
2. **The integrity of the verifier key**: a developer's `key.bin` is bound to a specific checkpoint and CommitLLM pin. A swapped key means swapped trust.
3. **The integrity of the CLI binary**: a user runs a binary that does what the README says. Distribution channel integrity matters.

### Adversary models

1. **Dishonest provider.** Substitutes model, quantization, or decode policy. CommitLLM's protocol is the defense. This project's job is to make using that defense ergonomic.
2. **Network attacker between client and provider.** Active MITM, TLS downgrade. Defense: HTTPS only; certificate validation enforced; no fallback to plaintext. Confirmed via test.
3. **Malicious receipt source.** A receipt blob handed to the user from an untrusted channel. Defense: receipt is rejected if it doesn't bind to the loaded key (model id, checkpoint hash, CommitLLM pin, key hash). Fail closed. ([04-error-model.md](./04-error-model.md))
4. **Tampered receipt in transit.** A receipt has been altered. Defense: CommitLLM verifier detects this; this project's tamper fuzz harness ([RFC-0009](../rfcs/RFC-0009-tamper-fuzz-harness.md)) ensures we don't have an integration regression that hides such failures.
5. **Compromised distribution channel.** An attacker publishes a fake `vi` binary or a fake provider image. Defense: GitHub Releases artifacts are signed and checksummed; Docker image is published with a documented digest; reproducible builds where feasible. See §"Distribution integrity" below.

### Adversary models out of scope

- **Side-channel attacks on the verifier on the client.** No timing-safety guarantees beyond what `subtle`/`constant-time` crates already provide for verification arithmetic.
- **Compromise of CommitLLM upstream itself.** We pin to a specific commit and document the pin; a compromised CommitLLM is a compromise of the protocol, not of this integration.
- **Compromise of HF or the model registry.** Mitigation: the keygen step records the checkpoint hash; subsequent receipts must bind to it. A switched checkpoint on HF is detected at keygen time (and only at keygen time).
- **Denial of service against the provider endpoint.** The provider is a demonstration deployment; it has rate limiting and a per-IP budget (see §"Provider abuse controls") but is not engineered for adversarial throughput.

## Trust boundaries

```
+-------------------+  trusted: nothing       +-------------------+
| Network           |  authority: TLS cert    | Network           |
+-------------------+                         +-------------------+
       ▲                                              ▲
       │ HTTPS                                        │ HTTPS
       │ + trace header                               │
+------┴------------+                         +-------┴-----------+
| Client            |  trust: pinned          | Provider          |
|  - vi CLI         |  CommitLLM lib,         |  - vLLM + CL pin  |
|  - vi-verifier    |  model checkpoint       |  - W8A8 ckpt      |
|  - key.bin (user  |  hash documented in     |  - GPU host       |
|    provisions)    |  keygen output          |                   |
+-------------------+                         +-------------------+
```

- The client trusts: the CommitLLM verifier crate at the pinned commit; the bound model checkpoint hash; the `key.bin` it loaded.
- The client does not trust: the provider's claims, the response payload, the receipt's content beyond what the verifier checks.
- The provider trusts: nothing about the client; treats the client as a public consumer with rate-limited access.

## Secrets

- **No project-managed secrets ship with the source tree.** Repo scanning is enforced by CI ([RFC-0013](../rfcs/RFC-0013-ci-pipeline.md)).
- **Provider-side secrets** (HF token for endpoint deployment, registry credentials) are injected via environment variables or via the deploy host's secret store. The repo contains `.env.example` only.
- **Client-side credentials** (API keys for self-hosted or HF-gated endpoints) are accepted via `--api-key` or `VI_API_KEY`. Never logged. Stripped from `args` before `process.start` emits.
- **Keys are not secrets.** The `key.bin` artifact is verification material, derived from public checkpoint bytes; it is freely distributable. The README must say so.

## Distribution integrity

- **GitHub Releases artifacts** for `vi` are uploaded with SHA-256 checksums alongside the binaries. A `verify-binary.sh` script (or platform-native equivalent) is documented.
- **Docker image** is published to a documented registry with a digest. The provider deployment recipe pins on digest, not on tag.
- **`cargo install`** integrity is delegated to the Rust toolchain and crates.io. We do not publish unverified mirrors.
- **No CI artifact** is used as a production artifact without explicit release tagging.

## Provider abuse controls

Demonstration endpoint, not SaaS. Controls are sensible defaults, not adversarial hardening.

- **Per-IP rate limit** on `POST /v1/chat/completions` (target: 60 requests / 5 minutes; tuned at deploy time).
- **Per-IP rate limit** on `POST /v1/audit` (target: 600 requests / 5 minutes; auditing should be cheaper for the user but it is not free).
- **Request body size limit**: 32 KiB on chat input. Prompts above the limit are 413 Payload Too Large with a JSON envelope.
- **Max tokens cap**: 1024 generated tokens per request. The provider clamps and notes in the response metadata.
- **Concurrency cap** at the container level: matches vLLM's `--max-num-seqs` configuration; defaults documented per GPU class.
- **No authentication required for v1 demo.** OQ-6 (public demo endpoint) will gate whether the demo endpoint exists; if it does, abuse controls above apply.

## CORS and origin policy (server-side)

- **Default `Access-Control-Allow-Origin: *`** on read-safe endpoints (`/healthz`).
- **No CORS on `POST /v1/chat/completions` or `POST /v1/audit`** by default. The browser is not the v1 client; CORS belongs with the v1.1 WASM verifier and is scoped there.

## Logging and PII

- Prompts and generated text are not PII in the demonstration context, but they are user content. The default is: prompt hashes only, never the prompt body, in logs. Same on the provider side: vLLM's request body logging is disabled by default in the provider image.
- The provider logs `request_id`, `model_id`, `commitllm_pin`, byte counts, durations. It does not log token contents.

## Cryptographic primitives in scope

- **SHA-256** for checkpoint hashing, key hashing, receipt hashing.
- **CRC32C** for binding-header integrity inside envelopes (not a security primitive; a corruption check).
- **TLS** on every network path. The CLI rejects HTTP URLs; the provider does not listen on HTTP at all.

## Cryptographic primitives out of scope

- **Signatures.** v1 does not sign receipts at the integration layer. CommitLLM's protocol carries its own commitments; we do not add a layer on top. The v1.2 batched compliance flow ([PRD §12](../../PRD.md)) will introduce a provider signature on the bundle.
- **Encryption of receipts.** Receipts are not confidential and do not need confidentiality at rest.

## Security testing in CI

Detailed in [07-testing-strategy.md](./07-testing-strategy.md):

- Tamper fuzz harness: 100 byte flips per PR, 1000 nightly. 100% rejection required.
- TLS rejection test: CLI refuses `http://` URLs.
- Binding mismatch test: a receipt bound to model A loaded against key for model B fails closed.
- Unknown version test: a receipt with `ver = 0xFE` fails closed.
- Repo secret scan: `gitleaks` (or equivalent) on every PR.

## Reporting

A `SECURITY.md` at the repo root documents:

- Where to report vulnerabilities.
- Expected response time.
- Coordinated disclosure policy.
- Out-of-scope items (corridor numbers being different from CommitLLM upstream is not a security report; CommitLLM protocol issues belong upstream).

The contents of `SECURITY.md` are tracked under a documentation issue.
