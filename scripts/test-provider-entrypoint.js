const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const net = require('node:net');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const entrypoint = path.join(root, 'provider', 'entrypoint.sh');

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
  assert.equal(boot.commitllm_pin, '25541e83');
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
  assert.equal(health.commitllm_pin, '25541e83');

  run.child.kill('SIGTERM');
  const result = await run.exit;
  assert.deepEqual(result, { code: 0, signal: null }, run.stderr());
  const shutdown = run.logs.findLast((entry) => entry.event === 'provider.shutdown');
  assert.ok(shutdown, 'missing shutdown log');
  assert.equal(shutdown.signal, 'SIGTERM');
  assert.equal(shutdown.exit_code, 0);
}

async function main() {
  await oneShotStubExitsCleanly();
  await sigtermShutsDownCleanly();
  console.log('Provider entrypoint contract tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
