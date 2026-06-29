const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv/dist/2020');

const { createServer } = require('../broker/server');

const root = path.resolve(__dirname, '..');
const schema = JSON.parse(fs.readFileSync(path.join(root, 'schemas', 'viex.schema.json'), 'utf8'));
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'fixtures', 'viex', 'manifest.json'), 'utf8'));
const ajv = new Ajv({ allErrors: true });
const validateBundle = ajv.compile(schema);

const rawBundleFieldNames = new Set(['prompt', 'raw_prompt', 'messages', 'answer', 'raw_answer', 'text']);
const credentialFieldNames = new Set([
  'api_key',
  'apikey',
  'api-key',
  'x_api_key',
  'openai_api_key',
  'anthropic_api_key',
  'access_token',
  'refresh_token',
  'authorization',
  'credential',
  'credentials',
  'password',
  'secret'
]);

function collectForbiddenFields(value, forbidden, trail = []) {
  if (!value || typeof value !== 'object') return [];
  if (Array.isArray(value)) {
    return value.flatMap((entry, index) => collectForbiddenFields(entry, forbidden, [...trail, `[${index}]`]));
  }

  const hits = [];
  for (const [key, child] of Object.entries(value)) {
    const next = [...trail, key];
    const normalized = key.toLowerCase().replace(/[\s-]/g, '_');
    if (forbidden.has(normalized)) hits.push(next.join('.').replaceAll('.[', '['));
    hits.push(...collectForbiddenFields(child, forbidden, next));
  }
  return hits;
}

function assertSchemaValid(bundle) {
  assert.equal(validateBundle(bundle), true, ajv.errorsText(validateBundle.errors));
}

function assertBundleOmitsRawText(bundle) {
  const rawFields = collectForbiddenFields(bundle, rawBundleFieldNames);
  assert.deepEqual(rawFields, [], `proof bundle contains raw fields: ${rawFields.join(', ')}`);
}

async function listen(server) {
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  return `http://127.0.0.1:${port}`;
}

async function jsonRequest(baseUrl, method, pathname, body, headers = {}) {
  const response = await fetch(`${baseUrl}${pathname}`, {
    method,
    headers: body === undefined ? headers : { 'content-type': 'application/json', ...headers },
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  const payload = await response.json();
  return { status: response.status, body: payload };
}

async function binaryRequest(baseUrl, method, pathname, body, headers = {}) {
  const response = await fetch(`${baseUrl}${pathname}`, {
    method,
    headers: body === undefined ? headers : { 'content-type': 'application/json', ...headers },
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  const payload = Buffer.from(await response.arrayBuffer());
  return { status: response.status, contentType: response.headers.get('content-type'), body: payload };
}

async function main() {
  let now = 1782734700000;
  const providerLogs = [];
  const server = createServer({ now: () => now, providerLog: (event) => providerLogs.push(event) });
  const baseUrl = await listen(server);

  try {
    const providersResponse = await jsonRequest(baseUrl, 'GET', '/providers');
    assert.equal(providersResponse.status, 200);
    assert.equal(providersResponse.body.providers.length, 2);

    const provider = providersResponse.body.providers.find((entry) => entry.provider_id === 'lab-a100-01');
    assert.ok(provider, 'expected lab-a100-01 provider');
    assert.equal(provider.model_id, manifest.reference.model_id);
    assert.equal(provider.checkpoint_hash, manifest.reference.checkpoint_hash);
    assert.equal(provider.commitllm_pin, manifest.reference.commitllm_pin);
    assert.deepEqual(provider.proof_modes, ['routine', 'deep']);
    assert.deepEqual(collectForbiddenFields(providersResponse.body, credentialFieldNames), []);

    const credentialHeaderResponse = await jsonRequest(baseUrl, 'GET', '/providers', undefined, {
      authorization: 'Bearer sk-test'
    });
    assert.equal(credentialHeaderResponse.status, 400);
    assert.match(credentialHeaderResponse.body.error, /credentials|API keys/i);

    const quoteRequest = {
      provider_id: provider.provider_id,
      model_id: provider.model_id,
      max_tokens: 128,
      decode_policy: { temperature: 0.2, top_p: 0.95 }
    };
    const quoteResponse = await jsonRequest(baseUrl, 'POST', '/quotes', quoteRequest);
    assert.equal(quoteResponse.status, 200);
    assert.equal(quoteResponse.body.provider_id, provider.provider_id);
    assert.equal(quoteResponse.body.model_id, provider.model_id);
    assert.equal(quoteResponse.body.checkpoint_hash, provider.checkpoint_hash);
    assert.equal(quoteResponse.body.key_hash, provider.key_hash);
    assert.equal(quoteResponse.body.commitllm_pin, provider.commitllm_pin);
    assert.equal(quoteResponse.body.decode_policy.max_tokens, 128);
    assert.equal(quoteResponse.body.estimated_price_usd, '0.001536');
    assert.match(quoteResponse.body.decode_policy_hash, /^sha256:[0-9a-f]{64}$/);
    assert.match(quoteResponse.body.signature, /^demo:qt_lab_a100_01_/);

    const credentialQuote = await jsonRequest(baseUrl, 'POST', '/quotes', {
      ...quoteRequest,
      api_key: 'sk-test'
    });
    assert.equal(credentialQuote.status, 400);
    assert.match(credentialQuote.body.error, /credentials|API keys/i);

    const malformedQuote = await jsonRequest(baseUrl, 'POST', '/quotes', null);
    assert.equal(malformedQuote.status, 400);
    assert.equal(malformedQuote.body.error, 'request body must be a JSON object');

    const unknownProviderQuote = await jsonRequest(baseUrl, 'POST', '/quotes', {
      provider_id: 'unknown-provider',
      model_id: provider.model_id,
      max_tokens: 128
    });
    assert.equal(unknownProviderQuote.status, 404);

    const wrongModelQuote = await jsonRequest(baseUrl, 'POST', '/quotes', {
      provider_id: provider.provider_id,
      model_id: 'qwen2.5-7b-w8a8',
      max_tokens: 128
    });
    assert.equal(wrongModelQuote.status, 400);

    const chatResponse = await jsonRequest(baseUrl, 'POST', '/chat', {
      quote_id: quoteResponse.body.quote_id,
      prompt: 'What causes rainbows?'
    });
    assert.equal(chatResponse.status, 200);
    assert.equal(chatResponse.body.quote_id, quoteResponse.body.quote_id);
    assert.match(chatResponse.body.request_id, /^req_lab_a100_01_/);
    assert.equal(typeof chatResponse.body.text, 'string');

    const bundle = chatResponse.body.proof_bundle;
    assertSchemaValid(bundle);
    assertBundleOmitsRawText(bundle);
    assert.equal(bundle.report.overall, 'not_run');
    assert.equal(bundle.quote.signature, quoteResponse.body.signature);
    assert.equal(bundle.quote.decode_policy_hash, quoteResponse.body.decode_policy_hash);
    assert.equal(bundle.request.max_tokens, quoteResponse.body.decode_policy.max_tokens);

    const auditResponse = await binaryRequest(
      baseUrl,
      'POST',
      '/v1/audit',
      {
        receipt_hash: bundle.receipt.sha256,
        tier: bundle.audit.tier,
        challenge: bundle.audit.challenge
      },
      { 'x-verifiable-intelligence-trace': 'trace-provider-123' }
    );
    assert.equal(auditResponse.status, 200);
    assert.equal(auditResponse.contentType, 'application/vnd.verifiable-intelligence.audit+binary');
    const auditBody = JSON.parse(auditResponse.body.toString('utf8'));
    assert.match(auditBody.request_id, /^aud_[0-9a-f]{8}$/);
    assert.equal(auditBody.receipt_hash, bundle.receipt.sha256);
    assert.equal(providerLogs.length, 1);
    assert.equal(providerLogs[0].event, 'provider.audit');
    assert.equal(providerLogs[0].trace_id, 'trace-provider-123');
    assert.equal(providerLogs[0].request_id, auditBody.request_id);
    assert.equal(providerLogs[0].tier, bundle.audit.tier);
    assert.equal(providerLogs[0].token_index, bundle.audit.challenge.token_index);
    assert.equal(providerLogs[0].layer_count, bundle.audit.challenge.layer_indices.length);
    assert.equal(typeof providerLogs[0].duration_ms, 'number');

    const credentialChat = await jsonRequest(baseUrl, 'POST', '/chat', {
      quote_id: quoteResponse.body.quote_id,
      prompt: 'What causes rainbows?',
      authorization: 'Bearer sk-test'
    });
    assert.equal(credentialChat.status, 400);

    const verifyResponse = await jsonRequest(baseUrl, 'POST', '/verify', { proof_bundle: bundle });
    assert.equal(verifyResponse.status, 200);
    assert.equal(verifyResponse.body.report.overall, 'pass');
    assert.ok(
      verifyResponse.body.report.checks.some((check) => check.id === 'decode_policy_binding'),
      'expected decode policy binding check'
    );

    const modelSwap = structuredClone(bundle);
    modelSwap.quote.model_id = 'qwen2.5-7b-w8a8';
    const modelSwapResponse = await jsonRequest(baseUrl, 'POST', '/verify', { proof_bundle: modelSwap });
    assert.equal(modelSwapResponse.status, 200);
    assert.equal(modelSwapResponse.body.report.overall, 'fail');
    assert.equal(modelSwapResponse.body.report.checks[0].field, 'quote.model_id');

    const decodePolicyTamper = structuredClone(bundle);
    decodePolicyTamper.quote.decode_policy_hash = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    const decodePolicyTamperResponse = await jsonRequest(baseUrl, 'POST', '/verify', {
      proof_bundle: decodePolicyTamper
    });
    assert.equal(decodePolicyTamperResponse.status, 200);
    assert.equal(decodePolicyTamperResponse.body.report.overall, 'fail');
    assert.equal(decodePolicyTamperResponse.body.report.checks[0].field, 'quote.decode_policy_hash');

    const receiptlessQuote = await jsonRequest(baseUrl, 'POST', '/quotes', {
      provider_id: 'desk-a10g-cheap',
      model_id: provider.model_id,
      max_tokens: 64,
      decode_policy: { temperature: 0.2, top_p: 0.95 }
    });
    assert.equal(receiptlessQuote.status, 200);
    const receiptlessChat = await jsonRequest(baseUrl, 'POST', '/chat', {
      quote_id: receiptlessQuote.body.quote_id,
      prompt: 'What causes rainbows?'
    });
    assert.equal(receiptlessChat.status, 200);
    assertSchemaValid(receiptlessChat.body.proof_bundle);
    const receiptlessVerify = await jsonRequest(baseUrl, 'POST', '/verify', {
      proof_bundle: receiptlessChat.body.proof_bundle
    });
    assert.equal(receiptlessVerify.body.report.overall, 'fail');
    assert.equal(receiptlessVerify.body.report.checks[0].field, 'receipt.encoding');

    now = quoteResponse.body.expires_unix_ms + 1;
    const expiredChat = await jsonRequest(baseUrl, 'POST', '/chat', {
      quote_id: quoteResponse.body.quote_id,
      prompt: 'What causes rainbows?'
    });
    assert.equal(expiredChat.status, 409);

    console.log('Broker API contract tests passed');
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
