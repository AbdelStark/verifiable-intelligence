# RFC-0016: Marketplace proof demo pivot

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-06-29
- Target milestone: v1 research demo

## Summary

v1 changes from a terminal-first Rust CLI/TUI reference app to a browser-first proof marketplace demo. The product spine is now: provider catalog, quote, prompt, response, CommitLLM receipt, proof bundle, browser verification, and clear red-path failures.

The old CLI/keygen/verifier work remains useful infrastructure. The old TUI is no longer a v1 requirement.

## Motivation

The strongest demo for verifiable inference is not another terminal command. It is a buyer asking a low-trust provider for an answer and getting a proof object that rejects model substitution. The user should see the model claim, price, prompt hash, answer hash, receipt, audit challenge, and final verdict in one browser page.

The motivating observation came from adversarial token markets. This project must not help operate unauthorized markets. It uses that setting only as a threat model for model substitution and dishonest provider claims.

## Goals

- Put the consumer proof flow in the first screen of the project.
- Make proof bundles portable and inspectable.
- Move browser verification to v1 scope.
- Reuse CommitLLM's maintained open-weight path rather than inventing a new protocol.
- Show red paths for fake model, prompt mismatch, answer rewrite, and receipt tamper.
- Preserve a lawful-use boundary in every public artifact.

## Non-Goals

- No unauthorized token resale, credential handling, or provider-term evasion.
- No closed-weight model support in v1.
- No production marketplace, real payment rail, escrow, or dispute workflow.
- No new cryptography.
- No claim that verification proves answer correctness.
- No terminal TUI release gate for v1.

## Proposed Design

### Product surface

The v1 demo is a browser app:

1. Buyer selects a provider from a catalog.
2. Buyer submits a prompt under a provider quote.
3. Provider returns text and a CommitLLM receipt.
4. App builds a `VIEX` proof bundle.
5. Browser verifier checks the bundle.
6. UI renders a proof card with pass/fail status and guarantee classes.

The initial app may be static with simulated fixtures. The live backend follows the same data contracts.

### Proof bundle

The proof bundle is the buyer artifact. It joins:

- provider quote,
- request binding,
- response binding,
- CommitLLM receipt envelope,
- verifier-key identity,
- audit reference,
- verification report.

The bundle is not a replacement for CommitLLM. It is packaging around CommitLLM so a buyer can verify market claims.

### Provider adapter

The provider keeps the OpenAI-compatible chat path and CommitLLM receipt opt-in:

- `POST /v1/chat/completions`
- request header `X-Verifiable-Receipt: 1`
- `POST /v1/audit`
- `GET /healthz`

The broker can wrap providers for the demo, but proof validity must not depend on trusting the broker.

### Model choice

The old PRD centered on Llama 3.2 1B Instruct W8A8 and a new corridor measurement. The pivot should prefer a CommitLLM-supported measured model first, such as Llama 3.1 8B W8A8 or Qwen2.5 7B W8A8. Smaller-model corridor work becomes research backlog unless upstream lands a maintained small profile.

### Browser verification

The v1 target is WASM verification in the browser. If CommitLLM verifier dependencies block WASM, the prototype may use a server-side verifier while the UI labels that path clearly. A server-side verifier is not a substitute for the final consumer story.

## Supersedes

This RFC supersedes the v1 role of:

- RFC-0008 TUI architecture,
- the v1.1 deferral of browser/WASM verification,
- terminal-first release gates SM-1 and SM-6 as written in the old PRD,
- Llama 3.2 1B corridor measurement as a v1 blocker.

It does not supersede:

- receipt format pinning,
- verifier-key binding,
- CommitLLM pinning,
- provider receipt header,
- provider audit endpoint,
- tamper fuzz harness,
- error taxonomy.

## Testing Strategy

- JSON Schema validation for `VIEX` bundles.
- Static browser smoke tests for happy path and each red path.
- Fixture-level verifier tests for prompt hash mismatch, answer hash mismatch, model/key mismatch, receipt tamper, and expired quote.
- WASM spike with measured binary size, memory use, and verification time.
- README/demo comprehension test covering open-weight-only, execution integrity, and lawful-use boundary.

## Rollout

1. Land pivot docs and static demo.
2. File a new GitHub epic and child issues.
3. Add proof bundle schema and fixtures.
4. Implement browser verifier spike.
5. Wire a live CommitLLM provider after the fixture flow is stable.
6. Close or mark old CLI/TUI issues as superseded only after the new epic is accepted.

## References

- [PRD.md](../../PRD.md)
- [SPEC.md](../../SPEC.md)
- [CommitLLM repository](https://github.com/lambdaclass/CommitLLM)
- [CommitLLM project site](https://commitllm.com/)
