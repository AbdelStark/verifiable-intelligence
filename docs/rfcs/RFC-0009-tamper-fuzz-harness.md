# RFC-0009: Tamper fuzz harness

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.2

## Summary

A deterministic per-PR harness flips a random single byte in a canonical fixture receipt 100 times, runs `vi verify` on each, and asserts that every run produces a typed error (either `corrupt_envelope`, `verification_failed`, or `identity_mismatch`) — never `pass`. A nightly job runs 1000 flips. The seed is recorded per run; a regression that reveals a missed flip captures the flipped byte offset and the unexpected output for triage. Achieving 100% rejection is a release gate ([SM-3](../../PRD.md)).

## Motivation

CommitLLM's verifier rejects tampered receipts; the project's risk ([PRD R6](../../PRD.md)) is that our integration code accidentally swallows a CommitLLM-detected failure (a logging bug, an error-mapping bug, a missing match arm). The cheapest way to catch that is a continuous fuzz over the byte range of a known-good receipt and a clear contract: every flip is detected.

## Goals

- 100% detection over per-PR 100-flip runs ([SM-3](../../PRD.md)).
- Reproducibility: every failure is reproducible from the recorded seed + flip offset.
- Coverage of every byte of the artifact, including envelope header bytes, binding header bytes, CommitLLM payload bytes, and trailing bytes.
- Fast enough to fit in a PR's CI budget (< 30 s per harness run on a CI runner).

## Non-Goals

- No multi-byte or structured mutation in v1. Single-byte flips are the minimum bar; structured mutations are a v1.x consideration if the protocol surface widens.
- No fuzz of the `vi keygen` path; that path's tamper surface is the checkpoint, which is hash-bound at keygen time.
- No fuzz of the audit payload path in v1's fuzz harness; the audit endpoint round-trip adds complexity not justified at v0.2.

## Proposed Design

### Fixture

- `tests/fixtures/canonical-receipt.bin`: a known-good receipt for a fixed prompt against the canonical key.
- `tests/fixtures/canonical-key.bin`: the matching key.
- Receipt is regenerated when the CommitLLM pin moves; regeneration is part of the pin-change checklist ([RFC-0011](./RFC-0011-commitllm-upstream-pinning.md)).

### Harness

A Rust integration test under `tests/tamper-fuzz.rs`:

```rust
#[test]
fn per_pr_tamper_fuzz() {
    let receipt = fs::read("tests/fixtures/canonical-receipt.bin").unwrap();
    let key = fs::read("tests/fixtures/canonical-key.bin").unwrap();

    let seed: u64 = std::env::var("TAMPER_FUZZ_SEED")
        .ok().and_then(|s| s.parse().ok())
        .unwrap_or_else(|| OsRng.next_u64());
    let n: usize = std::env::var("TAMPER_FUZZ_N")
        .ok().and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let mut rng = StdRng::seed_from_u64(seed);
    for i in 0..n {
        let mut bytes = receipt.clone();
        let offset = rng.gen_range(0..bytes.len());
        let bit = rng.gen_range(0..8);
        bytes[offset] ^= 1 << bit;

        let result = vi_verifier::verify(&bytes, &key, Tier::Routine);

        assert!(
            result.is_err(),
            "tamper-fuzz seed={} iter={} offset={} bit={} produced unexpected success",
            seed, i, offset, bit,
        );
        // Stronger assertion: the error category must be one of the expected.
        let category = result.err().unwrap().category();
        assert!(
            matches!(category,
                ErrorCategory::CorruptEnvelope
                | ErrorCategory::VerificationFailed
                | ErrorCategory::IdentityMismatch
                | ErrorCategory::UnknownVersion
            ),
            "tamper-fuzz seed={} iter={} offset={} bit={} produced category={:?}",
            seed, i, offset, bit, category,
        );
    }
}
```

### Per-PR run

- CI invokes `cargo test --test tamper-fuzz` on every PR.
- Default `N = 100`.
- Seed is printed on every run for reproducibility.
- A failure causes the seed and offsets to be captured in the CI artifact.

### Nightly run

- A GitHub Actions cron triggers `cargo test --test tamper-fuzz -- --ignored` (the nightly variant) with `TAMPER_FUZZ_N=1000`.
- A failure opens an issue automatically with the captured seed.

### Coverage discipline

The harness covers the entire receipt byte range. A flip at any offset must produce a typed error. Specifically:

| Offset range | Expected detection mechanism |
|--------------|------------------------------|
| Envelope magic (0..4) | `corrupt_envelope` (magic mismatch) |
| Envelope ver (4) | `unknown_version` (flips produce non-1) |
| Envelope flags (5) | `corrupt_envelope` (flags non-zero) |
| Binding header | `corrupt_envelope` (CRC32C fails) or `identity_mismatch` (CRC happens to match by coincidence — extremely rare; counts as detected by mismatch) |
| CommitLLM payload | `verification_failed` (cryptographic check fails) |

If a flip happens to land on a byte whose value passes coincidentally, the binding-CRC mismatch or the CommitLLM verifier still catches it. The harness asserts category membership, not exact category, to allow for this.

### Reporting

A successful harness run prints:

```
tamper-fuzz: seed=12345 N=100 all_rejected=true
```

A failed run prints the seed, iteration, offset, bit, and the result's category.

## Alternatives Considered

**Use AFL or libFuzzer for structured fuzzing.** Rejected: overkill for the v1 surface; single-byte flips suffice; AFL adds CI infrastructure burden.

**Run only nightly, not per PR.** Rejected: the harness is fast enough to run per PR; catching regressions at PR time is much better than discovering them next morning.

**Use the live provider instead of a fixture.** Rejected: introduces flakiness and cost; the fixture is the right level of test.

**Mutation outside the receipt (e.g., a mismatched key).** Rejected as a separate test; covered by the `identity_mismatch` integration test in [07-testing-strategy.md](../spec/07-testing-strategy.md).

## Drawbacks

- The fixture must be regenerated on every pin change. Mitigation: regeneration is a one-line script (`scripts/regenerate-fixtures.sh`), part of the pin-change checklist.
- A flaky failure due to nondeterminism in the verifier would be hard to debug. Mitigation: the seed is printed and reproducible; verification is deterministic by construction ([01-architecture.md §"Determinism guarantees"](../spec/01-architecture.md)).

## Migration / Rollout

- Lands in v0.2 alongside the verifier and the canonical fixture.
- The nightly cron lands the same week.
- Pre-v1.0 release gate: two consecutive weeks of clean per-PR runs and one week of clean nightlies.

## Testing Strategy

- The harness IS the test. Meta-testing: a deliberately broken verifier (returns `Ok(())` always) makes the harness fail — verified once during initial implementation, then guards against regression.

## Open Questions

None.

## References

- [PRD R6, SM-3, FR-15](../../PRD.md)
- [04-error-model.md](../spec/04-error-model.md)
- [RFC-0003 §"Tamper-defense pipeline"](./RFC-0003-receipt-format-pinning.md)
