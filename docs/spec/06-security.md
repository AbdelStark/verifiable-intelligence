# 06 - Security

This document states the marketplace-demo threat model. CommitLLM owns the protocol threat model; this project owns packaging, UI, broker, provider adapter, and distribution integrity.

## Assets

1. **Model-claim integrity.** The buyer can reject a response not bound to the quoted model/checkpoint/key.
2. **Prompt and answer binding.** The buyer can reject prompt changes and delivered-answer rewrites.
3. **Proof bundle integrity.** A shared `VIEX` bundle is either verifiable or fails closed.
4. **Verifier integrity.** Browser/CLI verifier code must match the published version and CommitLLM pin.
5. **Lawful-use boundary.** The project must not become credential resale infrastructure.

## Adversary models

- **Dishonest provider.** Advertises one model and serves another.
- **Dishonest broker.** Routes to a cheaper provider or mutates quote fields.
- **Post-processor.** Rewrites or cleans answer text after the committed path.
- **Bundle tamperer.** Edits quote, prompt hash, answer hash, receipt bytes, key hash, or audit fields.
- **Unsupported-model seller.** Claims closed-weight support without compatible proof material.

## Out of scope

- Stolen credentials, API-key pooling, and unauthorized resale are not supported.
- Side-channel attacks on browser verifier execution.
- Payment fraud, chargebacks, escrow, KYC, and sanctions controls.
- Compromise of CommitLLM upstream.
- Proving factual correctness or safety of the answer.

## Trust boundaries

- Browser trusts verifier code and selected verifier key.
- Browser does not trust provider claims, broker claims, or answer text.
- Broker is optional and not proof-critical.
- Provider trusts nothing about the buyer.
- Hosted demo never accepts user-supplied third-party API keys.

## Abuse controls

- No field or form for third-party API keys.
- No instructions for bypassing provider terms or rate limits.
- Public hosted demo uses project-owned credentials only.
- Live provider endpoints rate-limit chat and audit requests.
- Logs store prompt hashes, not raw prompts, by default.
- Unsupported closed-weight provider cards render as `unsupported`, not `unverified pass`.

## Distribution integrity

- Static demo artifacts are served from GitHub Pages or equivalent with release-tagged assets.
- Browser verifier WASM is checksum-pinned in release notes.
- CLI binaries, if published, carry SHA-256 checksums.
- Provider images pin by digest, not mutable tags.

## Security tests

- Proof bundle field tamper tests.
- Receipt byte tamper tests.
- Quote expiry tests.
- Model/key mismatch tests.
- Prompt hash mismatch tests.
- Answer rewrite tests.
- Unsupported closed-weight model tests.
- Browser smoke test that red paths cannot render as pass.
