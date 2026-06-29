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

const buyerGuide = 'docs/guides/buyer-proof-guide.md';
const providerGuide = 'docs/guides/provider-integration-guide.md';
const contributingGuide = 'CONTRIBUTING.md';

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

console.log('Documentation boundary checks passed');
