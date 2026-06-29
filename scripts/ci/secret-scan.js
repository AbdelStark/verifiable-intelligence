const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const ignoredDirectories = new Set([
  '.git',
  '.next',
  '.turbo',
  'node_modules',
  'target',
  '__pycache__',
]);
const ignoredExtensions = new Set([
  '.bin',
  '.jpg',
  '.jpeg',
  '.png',
  '.safetensors',
  '.wasm',
  '.zip',
]);

const patterns = [
  {
    name: 'private-key',
    regex: /-----BEGIN [A-Z ]*PRIVATE KEY-----/g,
  },
  {
    name: 'github-token',
    regex: /\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,255}\b/g,
  },
  {
    name: 'huggingface-token',
    regex: /\bhf_[A-Za-z0-9]{30,}\b/g,
  },
  {
    name: 'openai-token',
    regex: /\bsk-[A-Za-z0-9]{32,}\b/g,
  },
  {
    name: 'anthropic-token',
    regex: /\bsk-ant-[A-Za-z0-9_-]{32,}\b/g,
  },
  {
    name: 'aws-access-key',
    regex: /\bAKIA[0-9A-Z]{16}\b/g,
  },
  {
    name: 'slack-token',
    regex: /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/g,
  },
];

function shouldIgnore(relativePath) {
  const parts = relativePath.split(path.sep);
  if (parts.some((part) => ignoredDirectories.has(part))) {
    return true;
  }
  return ignoredExtensions.has(path.extname(relativePath).toLowerCase());
}

function isProbablyBinary(buffer) {
  return buffer.includes(0);
}

function lineNumberForOffset(text, offset) {
  return text.slice(0, offset).split(/\r?\n/).length;
}

function scanText(relativePath, text) {
  const findings = [];
  for (const pattern of patterns) {
    pattern.regex.lastIndex = 0;
    for (const match of text.matchAll(pattern.regex)) {
      findings.push({
        file: relativePath,
        line: lineNumberForOffset(text, match.index || 0),
        type: pattern.name,
      });
    }
  }
  return findings;
}

function walk(directory, findings = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    const relativePath = path.relative(root, absolutePath);
    if (shouldIgnore(relativePath)) {
      continue;
    }
    if (entry.isDirectory()) {
      walk(absolutePath, findings);
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    const buffer = fs.readFileSync(absolutePath);
    if (isProbablyBinary(buffer)) {
      continue;
    }
    findings.push(...scanText(relativePath, buffer.toString('utf8')));
  }
  return findings;
}

function runScan() {
  const findings = walk(root);
  if (findings.length) {
    for (const finding of findings) {
      console.error(`${finding.file}:${finding.line}: potential ${finding.type}`);
    }
    throw new Error(`secret scan found ${findings.length} potential secret(s)`);
  }
  console.log('secret scan passed');
}

function runSelfTest() {
  assert.deepEqual(scanText('docs/example.md', 'export HF_TOKEN="hf_..."'), []);
  assert.equal(
    scanText('leak.txt', `token=ghp_${'a'.repeat(36)}`).length,
    1,
    'GitHub token shape should be detected'
  );
  assert.equal(
    scanText('leak.txt', `token=hf_${'a'.repeat(30)}`).length,
    1,
    'Hugging Face token shape should be detected'
  );
  assert.equal(
    scanText('leak.txt', `-----BEGIN ${'PRIVATE'} KEY-----`).length,
    1,
    'private key header should be detected'
  );
  console.log('secret scan self-test passed');
}

if (require.main === module) {
  try {
    if (process.argv.includes('--self-test')) {
      runSelfTest();
    } else {
      runScan();
    }
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}

module.exports = {
  scanText,
};
