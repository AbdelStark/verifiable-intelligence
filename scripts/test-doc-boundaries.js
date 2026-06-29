const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function assertIncludes(file, text, label = text) {
  const body = read(file);
  if (!body.includes(text)) {
    throw new Error(`${file} missing ${label}`);
  }
}

function assertMatches(file, pattern, label = pattern.source) {
  const body = read(file);
  if (!pattern.test(body)) {
    throw new Error(`${file} missing ${label}`);
  }
}

function assertDeepEqual(actual, expected, label) {
  const actualJson = JSON.stringify(actual);
  const expectedJson = JSON.stringify(expected);
  if (actualJson !== expectedJson) {
    throw new Error(`${label} mismatch\nactual: ${actualJson}\nexpected: ${expectedJson}`);
  }
}

const buyerGuide = 'docs/guides/buyer-proof-guide.md';
const providerGuide = 'docs/guides/provider-integration-guide.md';
const contributingGuide = 'CONTRIBUTING.md';
const securityGuide = 'SECURITY.md';
const corridorGuide = 'docs/measurements/corridor.md';
const ciGuide = 'docs/ci/README.md';
const redBuildGuide = 'docs/ci/red-build.md';
const gpuRunnerGuide = 'docs/ci/gpu-runners.md';
const codeOfConduct = 'CODE_OF_CONDUCT.md';
const yankGuide = 'docs/release/yank.md';

assertIncludes('README.md', './docs/guides/buyer-proof-guide.md', 'buyer guide link');
assertIncludes('README.md', './docs/guides/provider-integration-guide.md', 'provider guide link');

for (const term of ['Exact', 'Algebraic', 'Statistical', 'Audited', 'Open']) {
  assertIncludes(buyerGuide, `| ${term} |`, `${term} proof-boundary row`);
}

for (const file of [buyerGuide, providerGuide, 'demo/index.html']) {
  assertMatches(file, /open-weight/i, 'open-weight-only boundary');
  assertMatches(file, /credential|API keys/i, 'credential boundary');
  assertMatches(file, /closed-weight/i, 'closed-weight boundary');
}

assertMatches(buyerGuide, /factually correct|answer is true/i, 'execution-integrity-only boundary');
assertMatches(providerGuide, /provider-term|provider terms|billing controls|rate limits/i, 'provider-term evasion boundary');
assertMatches(providerGuide, /server-side verification labeled as a fallback/i, 'server fallback boundary');

for (const term of [
  'Repository Layout',
  'Build',
  'Tests',
  'Updating Fixtures',
  'Bumping the CommitLLM Pin',
  'Pull Requests',
  'RFC-0001',
  'RFC-0009',
  'RFC-0011',
]) {
  assertIncludes(contributingGuide, term, `contributing guide ${term}`);
}

assertMatches(contributingGuide, /browser marketplace demo|browser proof-market demo/i, 'browser-first contributor scope');
assertMatches(contributingGuide, /unauthorized token resale/i, 'lawful-use contributor boundary');

for (const term of [
  'Reporting a Vulnerability',
  'Response SLOs',
  'Coordinated Disclosure',
  'Out of Scope',
  './PRD.md#6-non-goals',
]) {
  assertIncludes(securityGuide, term, `security policy ${term}`);
}

assertMatches(securityGuide, /3 business days/i, 'security acknowledgement SLO');
assertMatches(securityGuide, /7 calendar days/i, 'security triage SLO');
assertMatches(securityGuide, /unauthorized token resale/i, 'security lawful-use boundary');
assertIncludes('docs/spec/06-security.md', '../../SECURITY.md', 'security policy link');

for (const term of [
  'Corridor Measurement Template',
  'scripts/corridor/measure.py',
  'Reproducibility Tolerance',
  '0.0001',
  'reports/corridor/',
  'RFC-0010',
]) {
  assertIncludes(corridorGuide, term, `corridor template ${term}`);
}

const workflowDir = path.join(root, '.github', 'workflows');
const actualWorkflows = fs.readdirSync(workflowDir)
  .filter((name) => name.endsWith('.yml') || name.endsWith('.yaml'))
  .map((name) => `.github/workflows/${name}`)
  .sort();
const listedWorkflows = Array.from(read(ciGuide).matchAll(/`(\.github\/workflows\/[^`]+\.ya?ml)`/g))
  .map((match) => match[1])
  .sort();

assertDeepEqual(listedWorkflows, actualWorkflows, 'docs/ci workflow table');

for (const term of [
  'red-build.md',
  'gpu-runners.md',
  'reports/ci/',
  'reports/perf/',
  'release.yml',
  'corridor.yml',
  'deploy-hf.yml',
]) {
  assertIncludes(ciGuide, term, `ci guide ${term}`);
}

assertIncludes(ciGuide, 'cargo build -p verifiable-intelligence --no-default-features --locked', 'ci guide no-default build');
assertIncludes('docs/ci/performance.md', '--no-default-features', 'performance cli-only budget');
assertIncludes('docs/spec/08-performance-budget.md', '--no-default-features', 'spec cli-only budget');

for (const term of ['Red Build Runbook', 'Local Reproduction Map', 'gh run view', 'npm run test:bundle']) {
  assertIncludes(redBuildGuide, term, `red-build guide ${term}`);
}

for (const term of ['GPU Runner Setup', 'self-hosted', 'vi-corridor', 'cost', 'no workflow']) {
  assertIncludes(gpuRunnerGuide, term, `gpu runner guide ${term}`);
}

for (const term of [
  'Code of Conduct',
  'Our Pledge',
  'Enforcement',
  'Contributor Covenant, version 2.1',
]) {
  assertIncludes(codeOfConduct, term, `code of conduct ${term}`);
}

assertIncludes(contributingGuide, './CODE_OF_CONDUCT.md', 'code of conduct link');

for (const term of [
  'Release Yank Procedure',
  'cargo yank',
  'yanked-${VERSION}-${YANK_REASON}',
  'gh release edit',
  'Patch Release',
  'CHANGELOG.md',
]) {
  assertIncludes(yankGuide, term, `yank guide ${term}`);
}

console.log('Documentation boundary checks passed');
