const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function assertMatches(file, pattern, label = pattern.source) {
  const body = read(file);
  if (!pattern.test(body)) {
    throw new Error(`${file} missing ${label}`);
  }
}

function assertAll(file, checks) {
  for (const [pattern, label] of checks) {
    assertMatches(file, pattern, label);
  }
}

const boundaryChecks = [
  [/open-weight/i, 'open-weight-only boundary'],
  [/execution integrity|factually correct/i, 'execution-integrity-only boundary'],
  [/credential|API-key|unauthorized token resale/i, 'lawful-use boundary'],
  [/closed-weight/i, 'closed-weight unsupported boundary']
];

assertAll('README.md', [
  ...boundaryChecks,
  [/https:\/\/abdelstark\.github\.io\/verifiable-intelligence\//, 'hosted demo URL'],
  [/simulated provider responses|simulated fixtures/i, 'fixture disclosure']
]);

assertAll('demo/index.html', [
  ...boundaryChecks,
  [/simulated fixtures/i, 'fixture disclosure']
]);

assertAll('docs/release/v0.1.0-pivot-demo.md', [
  ...boundaryChecks,
  [/demo-v0\.1\.0/, 'release tag'],
  [/GitHub Pages/i, 'static hosting path'],
  [/Live provider calls are absent/i, 'live provider call disclosure'],
  [/simulated fixtures/i, 'simulated fixture disclosure'],
  [/A100-class host/i, 'GPU cost decision']
]);

assertMatches('.github/workflows/static-demo-pages.yml', /tags:\s*\n\s+- "demo-v\*"/, 'tag-only Pages deploy trigger');
assertMatches('.github/workflows/static-demo-pages.yml', /live_provider_calls": false/, 'release metadata live-call flag');

console.log('Release comprehension checks passed');
