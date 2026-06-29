const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv/dist/2020');

const root = path.resolve(__dirname, '..');
const fixtureDir = path.join(root, 'fixtures', 'viex');
const manifestPath = path.join(fixtureDir, 'manifest.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const schemaPath = path.resolve(fixtureDir, manifest.schema);
const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
const cliContracts = [
  {
    label: 'keygen output',
    schema: 'schemas/keygen-output.schema.json',
    fixtures: ['crates/vi-cli/tests/snapshots/output/keygen.json'],
  },
  {
    label: 'chat output',
    schema: 'schemas/chat-output.schema.json',
    fixtures: [
      'crates/vi-cli/tests/snapshots/output/chat.json',
      'fixtures/cli/chat-no-receipt.json',
    ],
  },
];

const ajv = new Ajv({ allErrors: true });
const validate = ajv.compile(schema);
const rawFieldNames = new Set(['prompt', 'raw_prompt', 'messages', 'answer', 'raw_answer', 'text']);

function collectRawFields(value, trail = []) {
  if (!value || typeof value !== 'object') return [];
  if (Array.isArray(value)) {
    return value.flatMap((entry, index) => collectRawFields(entry, [...trail, `[${index}]`]));
  }
  const hits = [];
  for (const [key, child] of Object.entries(value)) {
    const next = [...trail, key];
    if (rawFieldNames.has(key)) hits.push(next.join('.').replaceAll('.[', '['));
    hits.push(...collectRawFields(child, next));
  }
  return hits;
}

function firstFailingField(bundle) {
  const failed = bundle.report.checks.find((check) => check.status === 'fail');
  return failed && failed.field;
}

function checkReferenceBinding(bundle, fixtureFile) {
  if (!manifest.reference) return [];

  const referenceFailures = [];
  const expected = manifest.reference;
  const checks = [
    ['quote.model_id', bundle.quote.model_id, expected.model_id],
    ['quote.checkpoint_hash', bundle.quote.checkpoint_hash, expected.checkpoint_hash],
    ['quote.commitllm_pin', bundle.quote.commitllm_pin, expected.commitllm_pin],
    ['verifier.commitllm_pin', bundle.verifier.commitllm_pin, expected.commitllm_pin]
  ];

  for (const [field, actual, expectedValue] of checks) {
    if (actual !== expectedValue) {
      referenceFailures.push(`${fixtureFile}: expected ${field} ${expectedValue}, got ${actual}`);
    }
  }

  return referenceFailures;
}

const failures = [];
for (const fixture of manifest.fixtures) {
  const fixturePath = path.join(fixtureDir, fixture.file);
  const bundle = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
  if (!validate(bundle)) {
    failures.push(`${fixture.file}: schema errors: ${ajv.errorsText(validate.errors)}`);
    continue;
  }

  if (bundle.report.overall !== fixture.expected_overall) {
    failures.push(`${fixture.file}: expected overall ${fixture.expected_overall}, got ${bundle.report.overall}`);
  }

  failures.push(...checkReferenceBinding(bundle, fixture.file));

  const rawFields = collectRawFields(bundle);
  if (rawFields.length) {
    failures.push(`${fixture.file}: raw prompt/answer fields present: ${rawFields.join(', ')}`);
  }

  if (fixture.expected_failing_field) {
    const field = firstFailingField(bundle);
    if (field !== fixture.expected_failing_field) {
      failures.push(`${fixture.file}: expected failing field ${fixture.expected_failing_field}, got ${field || 'none'}`);
    }
  }
}

for (const contract of cliContracts) {
  const contractSchemaPath = path.join(root, contract.schema);
  const validateCliFixture = ajv.compile(JSON.parse(fs.readFileSync(contractSchemaPath, 'utf8')));
  for (const fixture of contract.fixtures) {
    const fixturePath = path.join(root, fixture);
    const value = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
    if (!validateCliFixture(value)) {
      failures.push(`${fixture}: schema errors: ${ajv.errorsText(validateCliFixture.errors)}`);
    }
  }
}

if (failures.length) {
  console.error(failures.join('\n'));
  process.exit(1);
}

console.log(`Validated ${manifest.fixtures.length} VIEX fixtures against ${path.relative(root, schemaPath)}`);
for (const contract of cliContracts) {
  console.log(
    `Validated ${contract.fixtures.length} ${contract.label} fixture(s) against ${contract.schema}`
  );
}
