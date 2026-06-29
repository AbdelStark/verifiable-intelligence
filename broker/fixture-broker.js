const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'fixtures', 'viex', 'manifest.json'), 'utf8'));
const reference = manifest.reference;

const verifiedProvider = {
  provider_id: 'lab-a100-01',
  display_name: 'Lab A100 01',
  model_id: reference.model_id,
  checkpoint_hash: reference.checkpoint_hash,
  key_hash: 'sha256:2b9f77c9a184df28b74cbd26e714b0e5b5ef72cc4d09514c18e4bc1a227f8001',
  commitllm_pin: reference.commitllm_pin,
  proof_modes: ['routine', 'deep'],
  price_per_1k_tokens_usd: '0.012',
  audit_endpoint: 'demo://providers/lab-a100-01/v1/audit',
  verifier_key_ref: 'demo://keys/lab-a100-01'
};

const receiptlessProvider = {
  provider_id: 'desk-a10g-cheap',
  display_name: 'Desk A10G cheap',
  model_id: reference.model_id,
  checkpoint_hash: reference.checkpoint_hash,
  key_hash: verifiedProvider.key_hash,
  commitllm_pin: reference.commitllm_pin,
  proof_modes: [],
  price_per_1k_tokens_usd: '0.004',
  audit_endpoint: 'demo://providers/desk-a10g-cheap/v1/audit',
  verifier_key_ref: 'demo://keys/lab-a100-01',
  receipt_available: false
};

const providers = [verifiedProvider, receiptlessProvider];
const secretFieldNames = new Set([
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

function sha256(value) {
  return `sha256:${crypto.createHash('sha256').update(value).digest('hex')}`;
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function containsCredentialField(value) {
  if (!value || typeof value !== 'object') return false;
  if (Array.isArray(value)) return value.some(containsCredentialField);
  return Object.entries(value).some(([key, child]) => {
    const normalized = key.toLowerCase().replaceAll(/[\s-]/g, '_');
    return secretFieldNames.has(normalized) || containsCredentialField(child);
  });
}

function isJsonObject(value) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function publicProvider(provider) {
  return {
    provider_id: provider.provider_id,
    display_name: provider.display_name,
    model_id: provider.model_id,
    checkpoint_hash: provider.checkpoint_hash,
    key_hash: provider.key_hash,
    commitllm_pin: provider.commitllm_pin,
    proof_modes: provider.proof_modes,
    price_per_1k_tokens_usd: provider.price_per_1k_tokens_usd
  };
}

function normalizeDecodePolicy(policy = {}) {
  return {
    temperature: Number.isFinite(policy.temperature) ? policy.temperature : 0.2,
    top_p: Number.isFinite(policy.top_p) ? policy.top_p : 0.95,
    max_tokens: Number.isInteger(policy.max_tokens) ? policy.max_tokens : undefined
  };
}

function estimatePrice(maxTokens, pricePer1k) {
  const estimated = (Number(pricePer1k) * (maxTokens / 1000)).toFixed(6);
  return estimated.replace(/0+$/, '').replace(/\.$/, '') || '0';
}

function answerFor(prompt, provider) {
  if (provider.receipt_available === false) {
    return 'Rainbows separate sunlight because water droplets refract and reflect different wavelengths at different angles.';
  }
  return 'Rainbows show ordered colors because water droplets refract, reflect, and disperse sunlight so each wavelength exits at a different angle.';
}

function failReport(id, checkClass, field, detail, now) {
  return {
    overall: 'fail',
    checked_at_unix_ms: now,
    checks: [{ id, class: checkClass, status: 'fail', field, detail }],
    warnings: ['prototype server verifier fallback; not browser-wasm'],
    unsupported: []
  };
}

function passReport(now) {
  return {
    overall: 'pass',
    checked_at_unix_ms: now,
    checks: [
      { id: 'quote_freshness', class: 'structural', status: 'pass', detail: 'quote is inside its validity window' },
      { id: 'model_binding', class: 'exact', status: 'pass', detail: 'quote model identity matches provider reference metadata and receipt identity' },
      { id: 'decode_policy_binding', class: 'exact', status: 'pass', detail: 'quote decode policy hash matches the receipt binding' },
      { id: 'prompt_binding', class: 'exact', status: 'pass', detail: 'request prompt hash matches the receipt binding' },
      { id: 'receipt_integrity', class: 'algebraic', status: 'pass', detail: 'receipt bytes match the expected receipt hash' },
      { id: 'answer_binding', class: 'exact', status: 'pass', detail: 'delivered answer hash matches the committed output' }
    ],
    warnings: ['prototype server verifier fallback; not browser-wasm'],
    unsupported: ['arbitrary-position attention outputs are audited, not verified']
  };
}

function createBroker({ now = () => Date.now() } = {}) {
  const quotes = new Map();

  function listProviders() {
    return { providers: providers.map(publicProvider) };
  }

  function createQuote(body) {
    if (!isJsonObject(body)) {
      return { status: 400, body: { error: 'request body must be a JSON object' } };
    }
    if (containsCredentialField(body)) {
      return { status: 400, body: { error: 'third-party API keys or credentials are not accepted' } };
    }

    const provider = providers.find((entry) => entry.provider_id === body.provider_id);
    if (!provider) return { status: 404, body: { error: 'unknown provider_id' } };
    if (body.model_id !== provider.model_id) {
      return { status: 400, body: { error: 'model_id does not match provider catalog' } };
    }
    if (!Number.isInteger(body.max_tokens) || body.max_tokens < 1 || body.max_tokens > 4096) {
      return { status: 400, body: { error: 'max_tokens must be an integer from 1 to 4096' } };
    }

    const decodePolicy = normalizeDecodePolicy({ ...body.decode_policy, max_tokens: body.max_tokens });
    const issuedAt = now();
    const quoteId = `qt_${provider.provider_id.replaceAll('-', '_')}_${issuedAt.toString(36)}`;
    const quote = {
      quote_id: quoteId,
      provider_id: provider.provider_id,
      model_id: provider.model_id,
      checkpoint_hash: provider.checkpoint_hash,
      key_hash: provider.key_hash,
      commitllm_pin: provider.commitllm_pin,
      decode_policy: decodePolicy,
      decode_policy_hash: sha256(canonicalJson(decodePolicy)),
      expires_unix_ms: issuedAt + 300000,
      estimated_price_usd: estimatePrice(body.max_tokens, provider.price_per_1k_tokens_usd),
      signature: `demo:${quoteId}:${provider.provider_id}:${provider.commitllm_pin}`
    };
    quotes.set(quoteId, { quote, provider });
    return { status: 200, body: quote };
  }

  function chat(body) {
    if (!isJsonObject(body)) {
      return { status: 400, body: { error: 'request body must be a JSON object' } };
    }
    if (containsCredentialField(body)) {
      return { status: 400, body: { error: 'third-party API keys or credentials are not accepted' } };
    }

    const entry = quotes.get(body.quote_id);
    if (!entry) return { status: 404, body: { error: 'unknown quote_id' } };
    if (entry.quote.expires_unix_ms <= now()) return { status: 409, body: { error: 'quote expired' } };
    if (typeof body.prompt !== 'string' || body.prompt.trim().length === 0) {
      return { status: 400, body: { error: 'prompt must be a non-empty string' } };
    }

    const requestId = `req_${body.quote_id.slice(3)}_${sha256(body.prompt).slice(7, 15)}`;
    const text = answerFor(body.prompt, entry.provider);
    const promptHash = sha256(body.prompt);
    const answerHash = sha256(text);
    const receiptMaterial = {
      request_id: requestId,
      model_id: entry.provider.model_id,
      prompt_hash: promptHash,
      answer_hash: answerHash,
      decode_policy_hash: entry.quote.decode_policy_hash,
      commitllm_pin: entry.provider.commitllm_pin
    };
    const receiptBytes = Buffer.from(canonicalJson(receiptMaterial), 'utf8');
    const receiptMissing = entry.provider.receipt_available === false;
    const checkedAt = now();
    const proofBundle = {
      magic: 'VIEX',
      schema_version: 1,
      created_unix_ms: checkedAt,
      quote: {
        quote_id: entry.quote.quote_id,
        provider_id: entry.quote.provider_id,
        model_id: entry.quote.model_id,
        checkpoint_hash: entry.quote.checkpoint_hash,
        commitllm_pin: entry.quote.commitllm_pin,
        key_hash: entry.quote.key_hash,
        decode_policy_hash: entry.quote.decode_policy_hash,
        price: {
          currency: 'USD',
          estimated: entry.quote.estimated_price_usd,
          per_1k_tokens: entry.provider.price_per_1k_tokens_usd
        },
        expires_unix_ms: entry.quote.expires_unix_ms,
        signature: entry.quote.signature
      },
      request: {
        request_id: requestId,
        prompt_hash: promptHash,
        input_spec_hash: sha256(`${entry.provider.model_id}:chat-template:v1`),
        max_tokens: entry.quote.decode_policy.max_tokens
      },
      response: {
        answer_hash: answerHash,
        answer_preview: text.slice(0, 180),
        generated_token_count: 31,
        output_spec_hash: sha256(`${entry.provider.model_id}:detokenize:v1`)
      },
      receipt: {
        encoding: receiptMissing ? 'missing' : 'base64',
        content_type: 'application/vnd.verifiable-intelligence.receipt+binary',
        ...(receiptMissing ? {} : { bytes_b64: receiptBytes.toString('base64') }),
        sha256: receiptMissing ? 'sha256:0000000000000000000000000000000000000000000000000000000000000000' : sha256(receiptBytes),
        size_bytes: receiptMissing ? 0 : receiptBytes.length
      },
      verifier: {
        key_hash: entry.provider.key_hash,
        key_ref: entry.provider.verifier_key_ref,
        commitllm_pin: entry.provider.commitllm_pin,
        verifier_version: 'fixture-broker-1',
        verification_mode: 'server'
      },
      audit: {
        audit_endpoint: entry.provider.audit_endpoint,
        tier: receiptMissing ? 'receipt-only' : 'routine',
        challenge: { token_index: 7, layer_indices: [3, 11, 19, 27] },
        payload_hash: receiptMissing ? 'sha256:0000000000000000000000000000000000000000000000000000000000000000' : sha256(`audit:${requestId}`)
      },
      report: {
        overall: 'not_run',
        checked_at_unix_ms: checkedAt,
        checks: [
          {
            id: 'prototype_verifier_pending',
            class: 'structural',
            status: 'not_run',
            detail: 'call POST /verify for the prototype server fallback report'
          }
        ],
        warnings: ['proof validity must be checked by verifier logic, not broker quote fields'],
        unsupported: []
      }
    };
    return { status: 200, body: { quote_id: entry.quote.quote_id, request_id: requestId, text, proof_bundle: proofBundle } };
  }

  function verify(body) {
    if (!isJsonObject(body)) {
      return { status: 400, body: { error: 'request body must be a JSON object' } };
    }
    if (containsCredentialField(body)) {
      return { status: 400, body: { error: 'third-party API keys or credentials are not accepted' } };
    }

    const bundle = body.proof_bundle;
    const checkedAt = now();
    if (!bundle || typeof bundle !== 'object') {
      return { status: 400, body: { error: 'proof_bundle is required' } };
    }
    const provider = providers.find((entry) => entry.provider_id === bundle.quote?.provider_id);
    if (!provider) return { status: 200, body: { report: failReport('provider_identity', 'structural', 'quote.provider_id', 'provider is not in broker catalog', checkedAt) } };
    if (bundle.quote.expires_unix_ms <= checkedAt) return { status: 200, body: { report: failReport('quote_freshness', 'structural', 'quote.expires_unix_ms', 'quote expired before verification time', checkedAt) } };
    if (bundle.quote.model_id !== provider.model_id) return { status: 200, body: { report: failReport('model_binding', 'exact', 'quote.model_id', 'quote model identity differs from provider reference metadata', checkedAt) } };
    if (bundle.quote.checkpoint_hash !== provider.checkpoint_hash) return { status: 200, body: { report: failReport('checkpoint_binding', 'exact', 'quote.checkpoint_hash', 'quote checkpoint hash differs from provider reference metadata', checkedAt) } };
    if (bundle.quote.commitllm_pin !== provider.commitllm_pin || bundle.verifier.commitllm_pin !== provider.commitllm_pin) {
      return { status: 200, body: { report: failReport('pin_binding', 'exact', 'quote.commitllm_pin', 'CommitLLM pin differs from provider reference metadata', checkedAt) } };
    }
    if (bundle.verifier.key_hash !== provider.key_hash || bundle.quote.key_hash !== provider.key_hash) {
      return { status: 200, body: { report: failReport('key_binding', 'exact', 'verifier.key_hash', 'verifier key hash does not match provider reference metadata', checkedAt) } };
    }
    if (bundle.receipt.encoding === 'missing') {
      return { status: 200, body: { report: failReport('receipt_presence', 'structural', 'receipt.encoding', 'provider did not return a CommitLLM receipt', checkedAt) } };
    }
    const receiptBytes = Buffer.from(bundle.receipt.bytes_b64 || '', 'base64');
    if (bundle.receipt.sha256 !== sha256(receiptBytes)) {
      return { status: 200, body: { report: failReport('receipt_integrity', 'algebraic', 'receipt.sha256', 'receipt bytes do not match receipt sha256', checkedAt) } };
    }

    let receipt;
    try {
      receipt = JSON.parse(receiptBytes.toString('utf8'));
    } catch {
      return { status: 200, body: { report: failReport('receipt_integrity', 'algebraic', 'receipt.bytes_b64', 'receipt bytes are not a decodable fixture receipt', checkedAt) } };
    }
    if (receipt.model_id !== bundle.quote.model_id) return { status: 200, body: { report: failReport('model_binding', 'exact', 'quote.model_id', 'receipt model identity differs from quote model identity', checkedAt) } };
    if (receipt.decode_policy_hash !== bundle.quote.decode_policy_hash) return { status: 200, body: { report: failReport('decode_policy_binding', 'exact', 'quote.decode_policy_hash', 'quote decode policy hash differs from receipt decode policy hash', checkedAt) } };
    if (receipt.prompt_hash !== bundle.request.prompt_hash) return { status: 200, body: { report: failReport('prompt_binding', 'exact', 'request.prompt_hash', 'request prompt hash differs from receipt prompt hash', checkedAt) } };
    if (receipt.answer_hash !== bundle.response.answer_hash) return { status: 200, body: { report: failReport('answer_binding', 'exact', 'response.answer_hash', 'response answer hash differs from receipt answer hash', checkedAt) } };
    if (receipt.commitllm_pin !== bundle.quote.commitllm_pin) return { status: 200, body: { report: failReport('pin_binding', 'exact', 'quote.commitllm_pin', 'receipt pin differs from quote pin', checkedAt) } };

    return { status: 200, body: { report: passReport(checkedAt) } };
  }

  return { listProviders, createQuote, chat, verify };
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
      if (body.length > 1024 * 1024) {
        reject(new Error('request body too large'));
        req.destroy();
      }
    });
    req.on('end', () => {
      if (!body) return resolve({});
      try {
        resolve(JSON.parse(body));
      } catch {
        reject(new Error('request body must be valid JSON'));
      }
    });
    req.on('error', reject);
  });
}

function sendJson(res, status, body) {
  res.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store'
  });
  res.end(JSON.stringify(body));
}

async function handleBrokerRequest(broker, req, res) {
  const url = new URL(req.url, 'http://127.0.0.1');
  try {
    if (containsCredentialField(req.headers)) {
      return sendJson(res, 400, { error: 'third-party API keys or credentials are not accepted' });
    }
    if (req.method === 'GET' && url.pathname === '/providers') {
      return sendJson(res, 200, broker.listProviders());
    }
    if (req.method === 'POST' && url.pathname === '/quotes') {
      const result = broker.createQuote(await readJsonBody(req));
      return sendJson(res, result.status, result.body);
    }
    if (req.method === 'POST' && url.pathname === '/chat') {
      const result = broker.chat(await readJsonBody(req));
      return sendJson(res, result.status, result.body);
    }
    if (req.method === 'POST' && url.pathname === '/verify') {
      const result = broker.verify(await readJsonBody(req));
      return sendJson(res, result.status, result.body);
    }
    return sendJson(res, 404, { error: 'not found' });
  } catch (error) {
    return sendJson(res, 400, { error: error.message });
  }
}

module.exports = {
  createBroker,
  handleBrokerRequest,
  sha256,
  canonicalJson
};
