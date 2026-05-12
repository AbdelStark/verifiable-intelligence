# RFC-0008: TUI architecture

- Status: Accepted
- Authors: AbdelStark
- Created: 2026-05-12
- Target milestone: v0.2

## Summary

The TUI is a `ratatui`-based terminal interface with three panes: prompt input (top), chat history (middle), verification panel (right). It drives the same `vi-client` and `vi-verifier` libraries the CLI does, surfacing the phase walk in real time. `--tamper byte-flip` deliberately corrupts the receipt before verification to produce a visible red failure for demonstrations. `--phase-delay <ms>` inserts deliberate per-phase delay so an audience can follow.

## Motivation

The Demo Audience persona ([PRD §3](../../PRD.md)) is the only persona for whom the protocol must be visible, not just correct. A verification that completes in 200 ms reads as a single instant; the cryptography is invisible. The TUI exists to slow the walk to human reading speed, surface each phase with a visual indicator, and show the green-vs-red transition that defines the integrity guarantee.

## Goals

- One terminal pane that shows phase transitions one at a time with green/red indicators.
- Works on any modern terminal (alacritty, kitty, iTerm2, GNOME Terminal, Windows Terminal) without special configuration.
- `--tamper byte-flip` produces a visibly different (red) result.
- `--phase-delay <ms>` inserts deliberate inter-phase delay for demo pacing.
- Total render overhead under 100 ms on top of the underlying verify time.

## Non-Goals

- No mouse input. Keyboard only.
- No theme system. One color palette, ANSI-compatible, with `--no-color` fallback.
- No mid-stream tampering. The tamper happens once, after the receipt is received, before verification.
- No multi-user / shared session capability.
- No persisted history. Each `vi tui` session is fresh.

## Proposed Design

### Layout

```
┌──────────────────────────────────────────────────────────────┐
│ vi tui — https://endpoint.example/v1 — pin=a1b2c3d4         │
├─────────────────────────────────┬────────────────────────────┤
│ Chat                            │ Verification               │
│                                 │                            │
│ > What causes rainbows?         │ [✓] embedding_merkle  3ms │
│                                 │ [✓] shell_freivalds   2ms │
│ Sunlight refracting in water... │ [✓] bridge_replay     8ms │
│                                 │ [✓] attention_corridor11ms│
│                                 │ [✓] kv_provenance     5ms │
│                                 │ [✓] lm_head           2ms │
│                                 │ [✓] decode_policy     1ms │
│                                 │                            │
│                                 │ overall: PASS  (32ms)      │
├─────────────────────────────────┴────────────────────────────┤
│ Prompt: _                                                    │
│ [F1] help  [F2] tamper  [F3] clear  [Esc] quit              │
└──────────────────────────────────────────────────────────────┘
```

Color discipline:

- Green for passed phase.
- Red for failed phase.
- Yellow for in-progress phase.
- Grey for not-yet-started.
- `--no-color`: use symbols (✓, ✗, …, ·) instead of color.

### Components

- **Event loop**: `tokio` with `crossterm` event polling.
- **Renderer**: `ratatui` with a custom widget for the phase walk.
- **Driver**: a state machine that holds:
  - prompt input buffer,
  - chat history (in-memory),
  - last receipt (if any),
  - current verification state (idle | in_phase(name) | done(report) | failed(phase, detail)).
- **Backend**: `vi-client` for the chat request, `vi-verifier` for verification. The TUI does NOT have its own verification logic; it observes the same crate the CLI uses.

### Phase walk implementation

The verifier emits structured phase-boundary events via a callback or channel. The TUI subscribes:

```rust
verifier.verify_with_callback(receipt, key, tier, |event| {
    match event {
        PhaseEvent::Started { phase } => tui.set_phase(phase, State::InProgress),
        PhaseEvent::Ended { phase, passed, detail, elapsed_ms } => {
            tui.set_phase(phase, if passed { State::Pass } else { State::Fail(detail) });
            if let Some(d) = phase_delay { tokio::time::sleep(d).await; }
        }
    }
});
```

The callback fires at the same boundaries the CLI's structured log emits ([05-observability.md](../spec/05-observability.md) §"Events the project owns"). The same source of truth feeds CLI logs and TUI rendering; they do not diverge.

### Tamper handling

`--tamper byte-flip`:

1. After the receipt is received from the provider, the TUI computes the receipt's byte length.
2. It selects one byte offset uniformly at random in `[0, receipt_len)`.
3. It flips one bit of that byte.
4. It writes the corrupted receipt to a temp path and runs verification against it.

The TUI shows a prominent banner: "TAMPERED: byte N, bit M". This is not a hidden state; the audience must know.

A keystroke (`F2`) toggles tamper for the next request without exiting the TUI. This is how a presenter shows "clean, then tampered" live.

### `--phase-delay`

- Default 0.
- A delay of 300–500 ms per phase is the documented demo value.
- The delay is purely UX; it never affects verification correctness or budgets in benchmarks (benchmarks run the CLI, not the TUI).

### Streaming chat

`vi chat` does not stream tokens to stdout in v1; the TUI **does** stream within its chat pane. The TUI's chat-pane streaming is implemented over the standard OpenAI streaming response (`stream: true`). The receipt arrives in the trailing multipart part once the stream ends.

### Accessibility

- Screen reader compatibility is not in v1 scope; the TUI is a visual artifact.
- `--no-color` provides symbol-based feedback.
- Terminal size detection: minimum 80×24; on smaller terminals, the TUI prints a friendly error and exits with code 2.

## Alternatives Considered

**Build the TUI on top of `crossterm` directly without `ratatui`.** Rejected: layout management is the bulk of the work; `ratatui` is maintained and minimizes the bespoke code.

**Use a web-based "TUI in a browser" via WASM.** Rejected: that's the v1.1 path. The TUI must run in a presenter's terminal during a talk where Wi-Fi may not exist.

**Make the TUI a thin shell over the CLI's JSON output.** Rejected: the CLI emits its result at the end; the TUI needs intermediate phase events. The right design is the verifier callback, which both CLI logs and TUI consume.

**Allow live tampering via mouse-drag on the receipt hex view.** Rejected: cute, but overengineered; `--tamper byte-flip` is enough for v1.

## Drawbacks

- `ratatui` and `crossterm` add ~1 MB to the binary. Acceptable; the `tui` feature can be disabled at build time per [RFC-0001](./RFC-0001-workspace-and-crate-layout.md).
- A buggy TUI render can mislead an audience. Mitigation: SM-6 gates release on a non-cryptographer comprehension test.

## Migration / Rollout

- TUI lands in v0.2 after the CLI loop is stable.
- The tamper demonstration is one of the v0.2 acceptance gates.

## Testing Strategy

- Mock-verifier tests: drive the TUI with a scripted sequence of phase events; assert rendering.
- `--phase-delay` honored: deterministic mock clock test.
- `--tamper byte-flip` end-to-end: against a real fixture receipt, the tampered version fails verification at some phase or as `corrupt_envelope`.
- Terminal-size test: 79×24 produces a friendly error.
- Snapshot tests of selected frames (using `ratatui`'s `Buffer` snapshot).
- Comprehension gate (SM-6) pre-release.

## Open Questions

None.

## References

- [02-public-api.md §1.2 `vi tui`](../spec/02-public-api.md)
- [PRD §6 Journey B](../../PRD.md)
- [RFC-0014 error taxonomy](./RFC-0014-error-taxonomy.md) for failure rendering.
