# Security Policy

`verifiable-intelligence` is a research proof of concept for verifiable
open-weight LLM inference. Please report issues that could make a buyer accept a
forged or misleading proof, leak secrets, or compromise the demo distribution
chain.

## Reporting a Vulnerability

Preferred channel: open a private vulnerability report through GitHub Security
Advisories:

<https://github.com/AbdelStark/verifiable-intelligence/security/advisories/new>

If that channel is unavailable, open a public GitHub issue that only asks for a
private reporting channel. Do not include exploit details, secrets, raw prompts,
private keys, API keys, or private proof bundles in a public issue.

Include:

- affected commit, release, or hosted demo URL,
- clear reproduction steps,
- expected and actual verifier result,
- proof bundle or fixture identifiers when they can be shared safely,
- impact assessment and whether the issue is already public.

## Response SLOs

This is a research demo, not a production security service, but reports should
still get predictable handling.

- Acknowledgement: within 3 business days.
- Initial triage: within 7 calendar days.
- Fix or mitigation plan: within 14 calendar days for confirmed issues.
- Critical integrity or secret-handling issues: best-effort mitigation as soon as
  practical, with public demo disablement if needed.

If a report affects CommitLLM itself, this project will help route it upstream,
but the upstream maintainers own protocol fixes.

## Coordinated Disclosure

Please give maintainers time to investigate before publishing exploit details.
The default disclosure window is 90 days after acknowledgement, or earlier by
mutual agreement once a fix, mitigation, or documented non-issue is available.

Public disclosure should include the affected versions, proof artifacts, impact,
mitigation, and any residual verification boundary. Do not publish private API
keys, raw prompts from another user, or private verifier material.

## In Scope

- Proof bundle validation errors that can turn a tampered `VIEX` bundle into a
  passing report.
- Model, checkpoint, key, prompt, decode-policy, answer, or CommitLLM-pin binding
  failures in project code.
- Browser verifier or CLI verifier behavior that accepts unsupported or malformed
  proof artifacts.
- Broker or provider adapter bugs that accept third-party credentials or mutate
  proof-critical fields without failing verification.
- Secret exposure in repository files, CI logs, release artifacts, demo assets, or
  provider configuration.
- Distribution integrity issues affecting published browser verifier, CLI, or
  provider artifacts.

## Out of Scope

The project non-goals in [`PRD.md#6-non-goals`](./PRD.md#6-non-goals) are not
valid vulnerability requests for this repository. In particular:

- requests to support unauthorized token resale, credential handling, API-key
  pooling, or provider-term evasion,
- requests to verify closed-weight models without compatible provider-published
  commitments or signed attestations,
- payment fraud, chargeback, escrow, KYC, sanctions, or marketplace dispute
  handling,
- claims that the model answer is factually wrong, unsafe, biased, or low
  quality, unless the report shows a proof-integrity failure,
- side-channel attacks on browser verifier execution,
- compromise or vulnerabilities inside CommitLLM upstream, except where this
  repository mishandles the pinned integration.

## Supported Versions

There is no production stable release yet. Security fixes target the current
`main` branch and the latest tagged research-demo release, if one exists.

## Safe Harbor

Good-faith research that stays within this policy, avoids privacy violations,
does not access or retain other users' data, and does not degrade hosted demo
availability will not be treated as malicious by this project.
