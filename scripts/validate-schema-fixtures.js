const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Ajv = require('ajv/dist/2020');

const root = path.resolve(__dirname, '..');
const viexFixtureDir = path.join(root, 'fixtures', 'viex');
const viexManifestPath = path.join(viexFixtureDir, 'manifest.json');
const viexManifest = JSON.parse(fs.readFileSync(viexManifestPath, 'utf8'));
const viexSchemaPath = path.resolve(viexFixtureDir, viexManifest.schema);
const viexSchema = JSON.parse(fs.readFileSync(viexSchemaPath, 'utf8'));
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
  {
    label: 'verify report',
    schema: 'schemas/verify-report.schema.json',
    fixtures: [
      'crates/vi-cli/tests/snapshots/output/verify.json',
      'fixtures/cli/verify-fail.json',
    ],
  },
];

const rawFieldNames = new Set(['prompt', 'raw_prompt', 'messages', 'answer', 'raw_answer', 'text']);

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), 'utf8'));
}

function compileSchema(ajv, relativePath) {
  return ajv.compile(readJson(relativePath));
}

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
  if (!viexManifest.reference) return [];

  const referenceFailures = [];
  const expected = viexManifest.reference;
  const checks = [
    ['quote.model_id', bundle.quote.model_id, expected.model_id],
    ['quote.checkpoint_hash', bundle.quote.checkpoint_hash, expected.checkpoint_hash],
    ['quote.commitllm_pin', bundle.quote.commitllm_pin, expected.commitllm_pin],
    ['verifier.commitllm_pin', bundle.verifier.commitllm_pin, expected.commitllm_pin],
  ];

  for (const [field, actual, expectedValue] of checks) {
    if (actual !== expectedValue) {
      referenceFailures.push(`${fixtureFile}: expected ${field} ${expectedValue}, got ${actual}`);
    }
  }

  return referenceFailures;
}

function validateValue(validate, value, label, ajv) {
  if (validate(value)) return [];
  return [`${label}: schema errors: ${ajv.errorsText(validate.errors)}`];
}

function validateViexFixtures(ajv) {
  const validateViex = ajv.compile(viexSchema);
  const failures = [];
  for (const fixture of viexManifest.fixtures) {
    const fixturePath = path.join(viexFixtureDir, fixture.file);
    const bundle = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
    const schemaFailures = validateValue(validateViex, bundle, fixture.file, ajv);
    if (schemaFailures.length) {
      failures.push(...schemaFailures);
      continue;
    }

    if (bundle.report.overall !== fixture.expected_overall) {
      failures.push(
        `${fixture.file}: expected overall ${fixture.expected_overall}, got ${bundle.report.overall}`
      );
    }

    failures.push(...checkReferenceBinding(bundle, fixture.file));

    const rawFields = collectRawFields(bundle);
    if (rawFields.length) {
      failures.push(`${fixture.file}: raw prompt/answer fields present: ${rawFields.join(', ')}`);
    }

    if (fixture.expected_failing_field) {
      const field = firstFailingField(bundle);
      if (field !== fixture.expected_failing_field) {
        failures.push(
          `${fixture.file}: expected failing field ${fixture.expected_failing_field}, got ${
            field || 'none'
          }`
        );
      }
    }
  }
  return failures;
}

function validateCliFixtures(ajv) {
  const failures = [];
  for (const contract of cliContracts) {
    const validateCliFixture = compileSchema(ajv, contract.schema);
    for (const fixture of contract.fixtures) {
      failures.push(...validateValue(validateCliFixture, readJson(fixture), fixture, ajv));
    }
  }
  return failures;
}

function validateAll() {
  const ajv = new Ajv({ allErrors: true });
  return [...validateViexFixtures(ajv), ...validateCliFixtures(ajv)];
}

function runSelfTest() {
  const ajv = new Ajv({ allErrors: true });
  const validateKeygen = compileSchema(ajv, 'schemas/keygen-output.schema.json');
  const validKeygen = readJson('crates/vi-cli/tests/snapshots/output/keygen.json');
  assert.deepEqual(
    validateValue(validateKeygen, validKeygen, 'valid keygen fixture', ajv),
    [],
    'valid keygen fixture should pass'
  );

  const brokenKeygen = { ...validKeygen, subcommand: 'chat' };
  const failures = validateValue(validateKeygen, brokenKeygen, 'broken keygen fixture', ajv);
  assert.equal(failures.length, 1, 'broken keygen fixture should fail schema validation');
  assert.match(failures[0], /broken keygen fixture/);
  console.log('schema fixture self-test passed');
}

function run() {
  const failures = validateAll();
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }

  console.log(
    `Validated ${viexManifest.fixtures.length} VIEX fixtures against ${path.relative(
      root,
      viexSchemaPath
    )}`
  );
  for (const contract of cliContracts) {
    console.log(
      `Validated ${contract.fixtures.length} ${contract.label} fixture(s) against ${contract.schema}`
    );
  }
}

if (process.argv.includes('--self-test')) {
  runSelfTest();
} else {
  run();
}
