const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv/dist/2020');

const root = path.resolve(__dirname, '..');
const schemaPath = path.join(root, 'schemas', 'error-envelope.schema.json');
const fixturesPath = path.join(root, 'fixtures', 'errors', 'envelopes.json');

const expectedExitCodes = new Map([
  ['verification_failed', 1],
  ['input', 2],
  ['network', 3],
  ['hash_mismatch', 4],
  ['receipt_missing', 5],
  ['unknown_version', 6],
  ['identity_mismatch', 7],
  ['unsupported_tier', 8],
  ['corrupt_envelope', 9],
  ['internal', 70],
]);

const ajv = new Ajv({ allErrors: true });
const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
const validate = ajv.compile(schema);

function validateEnvelope(envelope, label) {
  if (!validate(envelope)) {
    throw new Error(`${label}: ${ajv.errorsText(validate.errors)}`);
  }
  assert.equal(envelope.error, true, `${label}: error must be true`);
  assert.equal(
    envelope.exit_code,
    expectedExitCodes.get(envelope.category),
    `${label}: exit code must match category`
  );
}

function validateStaticFixtures() {
  const fixtures = JSON.parse(fs.readFileSync(fixturesPath, 'utf8'));
  assert.equal(fixtures.length, expectedExitCodes.size, 'one fixture per category is required');
  const seen = new Set();
  for (const fixture of fixtures) {
    validateEnvelope(fixture, `fixture ${fixture.category}`);
    seen.add(fixture.category);
  }
  assert.deepEqual(
    [...seen].sort(),
    [...expectedExitCodes.keys()].sort(),
    'fixtures must cover every category'
  );
}

function runRealEnvelope(category) {
  const result = childProcess.spawnSync(
    'cargo',
    ['run', '--quiet', '--locked', '-p', 'verifiable-intelligence', '--', '__error', category],
    {
      cwd: root,
      env: {
        ...process.env,
        VI_ENABLE_TEST_HOOKS: '1',
        VI_TRACE_ID: `schema-${category}`,
      },
      encoding: 'utf8',
    }
  );
  assert.equal(result.status, expectedExitCodes.get(category), `${category}: exit status`);
  assert.equal(result.stdout, '', `${category}: stdout should be empty`);
  return JSON.parse(result.stderr);
}

function validateRealEnvelopes() {
  for (const category of expectedExitCodes.keys()) {
    const envelope = runRealEnvelope(category);
    validateEnvelope(envelope, `real ${category}`);
    assert.equal(envelope.category, category, `${category}: category round trip`);
    assert.equal(envelope.trace_id, `schema-${category}`, `${category}: trace id round trip`);
  }
}

try {
  validateStaticFixtures();
  validateRealEnvelopes();
  console.log(`Validated ${expectedExitCodes.size} error fixture(s) and real CLI envelopes`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
