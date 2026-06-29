const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const entrypoint = path.join(root, 'provider', 'entrypoint.sh');
const commitllmShort = readCommitllmShort();

function readCommitllmShort() {
  const lock = fs.readFileSync(path.join(root, 'commitllm.lock'), 'utf8');
  const match = lock.match(/^commitllm_short\s*=\s*"([^"]+)"$/m);
  assert.ok(match, 'commitllm.lock must define commitllm_short');
  return match[1];
}

function zeroes(char) {
  return `sha256:${char.repeat(64)}`;
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  await new Promise((resolve) => server.close(resolve));
  return port;
}

function withTimeout(promise, label, ms = 10000) {
  let timeout;
  const timer = new Promise((_, reject) => {
    timeout = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, timer]).finally(() => clearTimeout(timeout));
}

async function providerPost(port, pathname, body, headers = {}) {
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...headers
    },
    body: typeof body === 'string' ? body : JSON.stringify(body)
  });
  const text = await response.text();
  let payload = null;
  try {
    payload = JSON.parse(text);
  } catch {
    payload = text;
  }
  return {
    status: response.status,
    contentType: response.headers.get('content-type'),
    body: payload
  };
}

function spawnEntrypoint(env) {
  const child = spawn(entrypoint, {
    cwd: root,
    env: {
      ...process.env,
      ...env
    },
    stdio: ['ignore', 'pipe', 'pipe']
  });
  const logs = [];
  let stderr = '';
  let stdout = '';
  let buffer = '';
  let resolveReady;
  let rejectReady;
  const ready = new Promise((resolve, reject) => {
    resolveReady = resolve;
    rejectReady = reject;
  });
  const exit = new Promise((resolve) => {
    child.on('exit', (code, signal) => resolve({ code, signal }));
  });

  child.stdout.on('data', (chunk) => {
    stdout += chunk;
  });
  child.stderr.on('data', (chunk) => {
    const text = chunk.toString('utf8');
    stderr += text;
    buffer += text;
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() || '';
    for (const line of lines) {
      if (!line.trim().startsWith('{')) continue;
      const event = JSON.parse(line);
      logs.push(event);
      if (event.event === 'provider.ready') resolveReady(event);
    }
  });
  child.on('error', rejectReady);
  child.on('exit', (code, signal) => {
    if (!logs.some((event) => event.event === 'provider.ready')) {
      rejectReady(new Error(`entrypoint exited before ready: code=${code} signal=${signal}\n${stderr}`));
    }
  });

  return {
    child,
    logs,
    ready: withTimeout(ready, 'provider.ready'),
    exit: withTimeout(exit, 'entrypoint exit'),
    stderr: () => stderr,
    stdout: () => stdout
  };
}

function event(logs, name) {
  const found = logs.find((entry) => entry.event === name);
  assert.ok(found, `missing ${name} log`);
  return found;
}

async function oneShotStubExitsCleanly() {
  const port = await freePort();
  const bind = `127.0.0.1:${port}`;
  const run = spawnEntrypoint({
    VI_BIND_ADDR: bind,
    VI_HEALTHZ_BIND_ADDR: bind,
    VI_PROVIDER_STUB: '1',
    VI_PROVIDER_STUB_EXIT_AFTER_READY: '1',
    VI_CHECKPOINT_HASH: zeroes('1'),
    VI_KEY_HASH: zeroes('2'),
    VI_LOG_LEVEL: 'debug'
  });

  const result = await run.exit;
  assert.deepEqual(result, { code: 0, signal: null });
  assert.equal(run.stdout(), '');
  const boot = event(run.logs, 'provider.boot');
  const ready = event(run.logs, 'provider.ready');
  const shutdown = event(run.logs, 'provider.shutdown');
  assert.equal(boot.model_id, 'llama-3.1-8b-w8a8');
  assert.equal(boot.commitllm_pin, commitllmShort);
  assert.equal(boot.checkpoint_hash, zeroes('1'));
  assert.equal(boot.key_hash, zeroes('2'));
  assert.equal(boot.max_num_seqs, 8);
  assert.equal(ready.healthz, `http://127.0.0.1:${port}/healthz`);
  assert.equal(shutdown.signal, 'stub_complete');
  assert.equal(shutdown.exit_code, 0);
}

async function sigtermShutsDownCleanly() {
  const port = await freePort();
  const bind = `127.0.0.1:${port}`;
  const run = spawnEntrypoint({
    VI_BIND_ADDR: bind,
    VI_HEALTHZ_BIND_ADDR: bind,
    VI_PROVIDER_STUB: '1',
    VI_CHECKPOINT_HASH: zeroes('3'),
    VI_KEY_HASH: zeroes('4')
  });

  await run.ready;
  const health = await fetch(`http://127.0.0.1:${port}/healthz`).then((response) => response.json());
  assert.equal(health.status, 'ok');
  assert.equal(health.model_id, 'llama-3.1-8b-w8a8');
  assert.equal(health.checkpoint_hash, zeroes('3'));
  assert.equal(health.key_hash, zeroes('4'));
  assert.equal(health.commitllm_pin, commitllmShort);

  const chat = await providerPost(port, '/v1/chat/completions', {
    messages: [{ role: 'user', content: 'hello' }],
    max_tokens: 4096
  });
  assert.equal(chat.status, 200);
  assert.equal(chat.body.verifiable_intelligence.max_tokens_requested, 4096);
  assert.equal(chat.body.verifiable_intelligence.max_tokens_effective, 1024);
  assert.equal(chat.body.verifiable_intelligence.max_tokens_clamped, true);

  const oversize = await providerPost(port, '/v1/chat/completions', JSON.stringify({ prompt: 'x'.repeat(33000) }));
  assert.equal(oversize.status, 413);
  assert.equal(oversize.body.error, true);
  assert.equal(oversize.body.detail.limit_bytes, 32768);

  run.child.kill('SIGTERM');
  const result = await run.exit;
  assert.deepEqual(result, { code: 0, signal: null }, run.stderr());
  const shutdown = run.logs.findLast((entry) => entry.event === 'provider.shutdown');
  assert.ok(shutdown, 'missing shutdown log');
  assert.equal(shutdown.signal, 'SIGTERM');
  assert.equal(shutdown.exit_code, 0);
}

async function rateLimitsApplyPerIp() {
  const port = await freePort();
  const bind = `127.0.0.1:${port}`;
  const run = spawnEntrypoint({
    VI_BIND_ADDR: bind,
    VI_HEALTHZ_BIND_ADDR: bind,
    VI_PROVIDER_STUB: '1',
    VI_RATE_LIMIT_RPM: '60',
    VI_AUDIT_RATE_LIMIT_RPM: '60',
    VI_RATE_LIMIT_WINDOW_S: '1'
  });

  await run.ready;
  const firstChat = await providerPost(
    port,
    '/v1/chat/completions',
    { messages: [], max_tokens: 8 },
    { 'x-forwarded-for': '203.0.113.7' }
  );
  const limitedChat = await providerPost(
    port,
    '/v1/chat/completions',
    { messages: [], max_tokens: 8 },
    { 'x-forwarded-for': '203.0.113.7' }
  );
  const otherIpChat = await providerPost(
    port,
    '/v1/chat/completions',
    { messages: [], max_tokens: 8 },
    { 'x-forwarded-for': '203.0.113.8' }
  );
  assert.equal(firstChat.status, 200);
  assert.equal(limitedChat.status, 429);
  assert.equal(limitedChat.body.category, 'rate_limit');
  assert.equal(otherIpChat.status, 200);

  const auditBody = {
    receipt_hash: zeroes('5'),
    tier: 'routine',
    challenge: { token_index: 7, layer_indices: [0, 2, 4] }
  };
  const firstAudit = await providerPost(port, '/v1/audit', auditBody, { 'x-forwarded-for': '203.0.113.9' });
  const limitedAudit = await providerPost(port, '/v1/audit', auditBody, { 'x-forwarded-for': '203.0.113.9' });
  assert.equal(firstAudit.status, 200);
  assert.equal(firstAudit.contentType, 'application/vnd.verifiable-intelligence.audit+binary');
  assert.equal(limitedAudit.status, 429);
  assert.equal(limitedAudit.body.category, 'rate_limit');

  run.child.kill('SIGTERM');
  const result = await run.exit;
  assert.deepEqual(result, { code: 0, signal: null }, run.stderr());
}

async function main() {
  await oneShotStubExitsCleanly();
  await sigtermShutsDownCleanly();
  await rateLimitsApplyPerIp();
  console.log('Provider entrypoint contract tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
