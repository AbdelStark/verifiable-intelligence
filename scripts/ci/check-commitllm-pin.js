const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const lockPath = path.join(root, 'commitllm.lock');

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function readLock() {
  const fields = {};
  for (const line of fs.readFileSync(lockPath, 'utf8').split(/\r?\n/)) {
    const match = line.match(/^([a-z_]+)\s*=\s*"([^"]+)"$/);
    if (match) {
      fields[match[1]] = match[2];
    }
  }
  return fields;
}

function assertPresent(value, label) {
  if (!value) {
    throw new Error(`commitllm.lock missing ${label}`);
  }
}

function assertIncludes(file, expected, label = expected) {
  const body = read(file);
  if (!body.includes(expected)) {
    throw new Error(`${file} missing ${label}`);
  }
}

function assertMatches(file, pattern, label) {
  const body = read(file);
  if (!pattern.test(body)) {
    throw new Error(`${file} missing ${label}`);
  }
}

const lock = readLock();
assertPresent(lock.commitllm, 'commitllm');
assertPresent(lock.commitllm_short, 'commitllm_short');

const fullSha = lock.commitllm.split('@')[1];
assertPresent(fullSha, 'full SHA in commitllm');
if (!/^[0-9a-f]{40}$/.test(fullSha)) {
  throw new Error(`CommitLLM SHA must be 40 lowercase hex chars, got ${fullSha}`);
}
if (lock.commitllm_short !== fullSha.slice(0, 8)) {
  throw new Error('commitllm_short must equal the first 8 chars of the full SHA');
}

for (const file of [
  'crates/vi-verifier/Cargo.toml',
  'verifier/wasm/Cargo.toml',
  'verifier/wasm/vendor/verilm-verify/Cargo.toml',
  'verifier/wasm/src/lib.rs',
  'provider/Dockerfile',
  'README.md'
]) {
  assertIncludes(file, fullSha, 'full CommitLLM SHA');
}

assertIncludes('README.md', lock.commitllm_short, 'short CommitLLM SHA');

const dockerfile = read('provider/Dockerfile');
const dockerArgs = Object.fromEntries(
  Array.from(dockerfile.matchAll(/^ARG\s+([A-Z0-9_]+)=([^\s]+)$/gm)).map((match) => [
    match[1],
    match[2]
  ])
);
const fromLines = dockerfile
  .split(/\r?\n/)
  .filter((line) => line.trim().toUpperCase().startsWith('FROM '));

if (fromLines.length < 2) {
  throw new Error('provider/Dockerfile must have builder and runtime FROM stages');
}
for (const line of fromLines) {
  const resolved = line.replace(/\$\{([A-Z0-9_]+)\}/g, (_, name) => dockerArgs[name] || '');
  if (!/@sha256:[0-9a-f]{64}\b/.test(resolved)) {
    throw new Error(`provider/Dockerfile FROM line is not digest pinned: ${line}`);
  }
}

assertMatches(
  'provider/Dockerfile',
  new RegExp(`^ARG COMMITLLM_SHA=${fullSha}$`, 'm'),
  'COMMITLLM_SHA build arg matching commitllm.lock'
);
assertMatches('provider/Dockerfile', /\bAS provider-builder\b/, 'provider-builder stage');
assertMatches('provider/Dockerfile', /\bAS provider-runtime\b/, 'provider-runtime stage');
assertMatches(
  'provider/Dockerfile',
  /pip wheel\s+\\\n\s+--no-deps\s+\\\n\s+--wheel-dir \/opt\/wheelhouse\s+\\\n\s+\/opt\/commitllm\/sidecar/m,
  'CommitLLM sidecar wheel build'
);
assertIncludes(
  'provider/Dockerfile',
  'COPY --from=provider-builder /opt/wheelhouse /opt/wheelhouse',
  'runtime wheelhouse copy'
);

console.log(`CommitLLM pin checks passed: ${lock.commitllm}`);
