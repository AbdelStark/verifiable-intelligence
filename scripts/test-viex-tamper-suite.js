const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv/dist/2020');

const root = path.resolve(__dirname, '..');
const schema = JSON.parse(fs.readFileSync(path.join(root, 'schemas', 'viex.schema.json'), 'utf8'));
const baseline = JSON.parse(fs.readFileSync(path.join(root, 'fixtures', 'viex', 'happy-path.json'), 'utf8'));
const ajv = new Ajv({ allErrors: true });
const validate = ajv.compile(schema);

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function check(id, checkClass, status, field, detail) {
  const result = { id, class: checkClass, status, detail };
  if (field) result.field = field;
  return result;
}

function failed(id, checkClass, field, detail) {
  return {
    overall: 'fail',
    checked_at_unix_ms: baseline.report.checked_at_unix_ms,
    checks: [check(id, checkClass, 'fail', field, detail)],
    warnings: [],
    unsupported: []
  };
}

function passedReport() {
  return {
    overall: 'pass',
    checked_at_unix_ms: baseline.report.checked_at_unix_ms,
    checks: [
      check('quote_freshness', 'structural', 'pass', null, 'quote is inside its validity window'),
      check('model_binding', 'exact', 'pass', null, 'quote model identity matches the receipt and verifier key'),
      check('prompt_binding', 'exact', 'pass', null, 'request prompt hash matches the receipt binding'),
      check('receipt_integrity', 'algebraic', 'pass', null, 'receipt bytes match the expected receipt hash'),
      check('answer_binding', 'exact', 'pass', null, 'delivered answer hash matches the committed output')
    ],
    warnings: [],
    unsupported: []
  };
}

function verifyStructurally(bundle) {
  if (!validate(bundle)) {
    return failed('schema_validation', 'structural', 'bundle', ajv.errorsText(validate.errors));
  }

  if (bundle.quote.expires_unix_ms <= bundle.report.checked_at_unix_ms) {
    return failed('quote_freshness', 'structural', 'quote.expires_unix_ms', 'quote expired before verification time');
  }
  if (bundle.quote.model_id !== baseline.quote.model_id) {
    return failed('model_binding', 'exact', 'quote.model_id', 'quote model identity differs from the expected receipt identity');
  }
  if (bundle.quote.checkpoint_hash !== baseline.quote.checkpoint_hash) {
    return failed('checkpoint_binding', 'exact', 'quote.checkpoint_hash', 'quote checkpoint hash differs from the verifier key binding');
  }
  if (bundle.quote.commitllm_pin !== baseline.quote.commitllm_pin) {
    return failed('pin_binding', 'exact', 'quote.commitllm_pin', 'quote CommitLLM pin differs from the verifier pin');
  }
  if (bundle.verifier.key_hash !== bundle.quote.key_hash) {
    return failed('key_binding', 'exact', 'verifier.key_hash', 'verifier key hash does not match quote key hash');
  }
  if (bundle.request.prompt_hash !== baseline.request.prompt_hash) {
    return failed('prompt_binding', 'exact', 'request.prompt_hash', 'request prompt hash differs from the receipt prompt hash');
  }
  if (bundle.response.answer_hash !== baseline.response.answer_hash) {
    return failed('answer_binding', 'exact', 'response.answer_hash', 'displayed answer hash differs from the committed answer hash');
  }
  if (bundle.receipt.bytes_b64 !== baseline.receipt.bytes_b64) {
    return failed('receipt_integrity', 'algebraic', 'receipt.bytes_b64', 'receipt bytes differ from the committed receipt payload');
  }

  return passedReport();
}

const validHash = (char) => `sha256:${char.repeat(64)}`;
const cases = [
  {
    name: 'quote model ID mutation',
    expectedField: 'quote.model_id',
    mutate: (bundle) => {
      bundle.quote.model_id = 'qwen2.5-7b-w8a8';
    }
  },
  {
    name: 'checkpoint hash mutation',
    expectedField: 'quote.checkpoint_hash',
    mutate: (bundle) => {
      bundle.quote.checkpoint_hash = validHash('e');
    }
  },
  {
    name: 'key hash mutation',
    expectedField: 'verifier.key_hash',
    mutate: (bundle) => {
      bundle.verifier.key_hash = validHash('c');
    }
  },
  {
    name: 'prompt hash mutation',
    expectedField: 'request.prompt_hash',
    mutate: (bundle) => {
      bundle.request.prompt_hash = validHash('a');
    }
  },
  {
    name: 'answer hash mutation',
    expectedField: 'response.answer_hash',
    mutate: (bundle) => {
      bundle.response.answer_hash = validHash('b');
    }
  },
  {
    name: 'CommitLLM pin mutation',
    expectedField: 'quote.commitllm_pin',
    mutate: (bundle) => {
      bundle.quote.commitllm_pin = 'abcdef1';
    }
  },
  {
    name: 'receipt bytes mutation',
    expectedField: 'receipt.bytes_b64',
    mutate: (bundle) => {
      bundle.receipt.bytes_b64 = 'VklSQwEAdGFtcGVyZWQ=';
    }
  },
  {
    name: 'expired quote mutation',
    expectedField: 'quote.expires_unix_ms',
    mutate: (bundle) => {
      bundle.quote.expires_unix_ms = bundle.report.checked_at_unix_ms - 1;
    }
  }
];

const failures = [];
const passReport = verifyStructurally(clone(baseline));
if (passReport.overall !== 'pass') {
  failures.push(`baseline should pass, got ${passReport.overall}: ${JSON.stringify(passReport.checks[0])}`);
}

for (const testCase of cases) {
  const bundle = clone(baseline);
  testCase.mutate(bundle);
  const report = verifyStructurally(bundle);
  const failedCheck = report.checks.find((entry) => entry.status === 'fail');
  if (report.overall !== 'fail') {
    failures.push(`${testCase.name}: expected fail, got ${report.overall}`);
    continue;
  }
  if (!failedCheck) {
    failures.push(`${testCase.name}: report did not include a failed check`);
    continue;
  }
  if (failedCheck.field !== testCase.expectedField) {
    failures.push(`${testCase.name}: expected first failing field ${testCase.expectedField}, got ${failedCheck.field || 'none'}`);
  }
}

if (failures.length) {
  console.error(failures.join('\n'));
  process.exit(1);
}

console.log(`Verified ${cases.length} VIEX tamper mutations fail with named first fields`);
