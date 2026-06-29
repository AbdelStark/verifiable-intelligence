# 10 - Glossary

Canonical terms used after the marketplace-demo pivot.

## Protocol terms

**Audit endpoint.** Provider HTTP path that returns a CommitLLM audit payload for a challenge.

**CommitLLM pin.** Exact upstream commit SHA used by provider and verifier.

**Decode policy.** Sampling and output policy used to produce the delivered answer.

**Delivered answer binding.** Verification that the displayed answer matches the committed output path.

**Proof bundle.** A `VIEX` JSON artifact that packages market context plus CommitLLM verification material. It is not a new protocol.

**Receipt.** CommitLLM binary artifact returned with an answer when the client requests verifiable inference.

**Verifier key.** Public verification material bound to a model checkpoint and CommitLLM pin.

## Market-demo terms

**Authorized provider.** A provider operating an open-weight model it is allowed to serve. v1 supports only this class.

**Broker.** Optional demo service that lists providers, issues quotes, proxies chat requests, and assembles proof bundles. It is not trusted for proof validity.

**Consumer buyer.** The user who wants to verify what model produced the purchased response.

**Fake-model provider.** Demo red-path provider that claims one model and returns evidence for another.

**Quote.** Short-lived provider claim containing model identity, checkpoint hash, key hash, price, decode policy, expiry, and signature.

**`VIEX`.** Magic string for the buyer-facing proof bundle.

## Guarantee classes

**Exact.** Deterministic check against committed material.

**Algebraic.** Freivalds-style or equivalent randomized algebraic check with verifier-secret randomness.

**Statistical.** Sampled check where coverage depends on challenge selection.

**Audited.** Data is committed and opened for inspection, but the computation may not be independently re-executed for every position.

**Open.** Known unsupported or unresolved claim. The UI must not render this as verified.

**Structural.** Schema, hash, expiry, identity, or version check around the protocol artifact.

## Excluded vocabulary

Avoid these in specs, issues, commits, code comments, and user-facing text:

- "Unauthorized token resale" or "credential resale" as project capabilities.
- "Verified Claude/GPT/Gemini" unless a compatible provider attestation exists.
- "Guaranteed correct answer".
- "Tamper-proof".
- "ZK verified" or "zero-knowledge" except explicit contrast.
- "Proof" without context when the statement is only a structural or statistical check.
