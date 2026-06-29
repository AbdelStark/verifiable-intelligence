# GPU Runner Setup

Current status: no workflow in `.github/workflows/` requires a self-hosted GPU
runner. RFC-0013 reserves GPU runners for future corridor measurements and other
manual release-quality checks.

Do not add a GPU-backed workflow until the workflow can exit clearly when no
runner is configured and the cost owner has approved the run path.

## Intended Use

GPU runners are for manual or scheduled measurement work that cannot run on
standard GitHub-hosted runners:

- CommitLLM corridor measurement,
- live provider smoke tests against a real open-weight checkpoint,
- no-cache provider image checks that require GPU libraries.

Per-PR proof-bundle, browser, Rust, schema, and docs checks should stay on
GitHub-hosted runners.

## Runner Requirements

- Linux x86_64 host with one supported GPU class: A10G, L4, or A100.
- NVIDIA driver and CUDA version recorded in the measurement output.
- Docker and NVIDIA Container Toolkit installed when provider containers are
  tested.
- Network egress limited to required package registries, model artifact hosts,
  and GitHub.
- Ephemeral workspace cleanup between jobs.
- No long-lived model secrets, provider credentials, or buyer prompts written to
  runner disk outside the job workspace.

Use labels that make scheduling explicit:

```text
self-hosted
linux
x64
gpu
vi-corridor
gpu-a10g | gpu-l4 | gpu-a100
```

## Setup Checklist

1. Create a GitHub runner group for this repository or organization.
2. Register the runner with the labels above.
3. Install Rust, Node.js, Python, Docker, NVIDIA driver, CUDA, and NVIDIA
   Container Toolkit versions required by the measurement script.
4. Verify `nvidia-smi`, `docker run --gpus all`, `cargo --version`, and
   `node --version` before enabling workflow dispatch.
5. Run a no-secret smoke job first.
6. Add cost and timeout limits to the workflow before running a full measurement.
7. Upload measurement output as artifacts and open a PR with committed reports
   only after review.

## Security And Cost Rules

- Prefer ephemeral cloud instances that are destroyed after the run.
- Never run untrusted pull-request code on a privileged self-hosted GPU runner.
- Do not mount host credential directories into provider containers.
- Do not log raw prompts, private proof bundles, API keys, or registry tokens.
- Set job timeouts and concurrency limits to prevent runaway GPU spend.

## Decommissioning

When a runner is no longer needed:

1. Remove it from the GitHub runner group.
2. Revoke registration tokens and cloud credentials.
3. Destroy the instance or wipe the disk.
4. Close or update any issue that depended on that runner.
