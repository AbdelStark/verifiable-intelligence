# 01 - Architecture

## System view

```
Consumer browser
  demo app
  proof card
  WASM verifier target
        |
        | HTTPS
        v
Demo broker (optional)
  provider catalog
  quote API
  proof bundle assembly
        |
        | OpenAI-compatible chat + receipt opt-in
        v
Provider
  CommitLLM-instrumented serving
  open-weight W8A8 model
  receipt emission
  audit endpoint
```

The broker improves demo ergonomics but is not trusted for proof validity. The proof bundle must remain verifiable from provider quote, receipt, verifier material, and audit data.

## Components

### Browser-side

| Component | Responsibility |
|-----------|----------------|
| Static demo app | Provider catalog, quote state, prompt input, response, proof card, red-path toggles |
| Proof bundle inspector | Renders `VIEX` fields and guarantee classes |
| Browser verifier | WASM target for CommitLLM verification; may be stubbed in the static prototype |
| Download/share surface | Exports proof bundle JSON |

### Broker-side (optional)

| Component | Responsibility |
|-----------|----------------|
| Provider catalog | Lists authorized demo providers and their verifier-key identities |
| Quote service | Issues short-lived quotes for model, price, decode policy, and terms |
| Chat proxy | Sends receipt opt-in requests to the selected provider |
| Bundle assembler | Packages quote, response binding, receipt, and report |
| Verifier API | Prototype fallback if browser WASM is blocked |

### Provider-side

| Component | Responsibility |
|-----------|----------------|
| CommitLLM prover | Emits receipts and answers audit challenges |
| OpenAI-compatible chat | Serves `POST /v1/chat/completions` |
| Audit endpoint | Serves `POST /v1/audit` |
| Health endpoint | Advertises model ID, checkpoint hash, CommitLLM pin, and key hash |

### Rust utilities

The inherited Rust CLI/keygen/verifier crates remain useful for:

- generating verifier keys,
- validating fixtures,
- running CI checks,
- debugging provider integrations,
- offering a power-user verification path.

They are not the primary v1 demo surface.

## Data flow: verified market response

1. Browser loads the provider catalog.
2. Buyer selects a provider and enters a prompt.
3. App or broker creates a quote with model ID, checkpoint hash, price, expiry, decode policy, and verifier-key hash.
4. Provider receives chat request with `X-Verifiable-Receipt: 1`.
5. Provider returns answer plus CommitLLM receipt.
6. App computes prompt hash and delivered-answer hash.
7. App packages a `VIEX` proof bundle.
8. Browser verifier checks quote freshness, provider identity, receipt binding, prompt hash, answer hash, model/key identity, and selected audit challenge.
9. UI renders pass/fail and names unsupported or open guarantee classes.

## Trust boundaries

- The buyer does not trust the provider's prose claims.
- The buyer does not trust the broker for proof validity.
- The buyer trusts the verifier code, CommitLLM pin, and verifier key identity they chose.
- The provider does not trust the buyer.
- The demo does not trust user-supplied third-party API keys because it never accepts them.

## Out of scope at the architecture layer

- No closed-weight provider adapters.
- No payment rail.
- No broker custody, escrow, or dispute resolution.
- No background browser extension.
- No protocol changes to CommitLLM.
