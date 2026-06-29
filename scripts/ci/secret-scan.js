const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const allowlistPath = path.join(root, 'secret-scan.allowlist.json');
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
const millisecondsPerDay = 24 * 60 * 60 * 1000;

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

function fingerprintMatch(value) {
  return `sha256:${crypto.createHash('sha256').update(value).digest('hex')}`;
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
        fingerprint: fingerprintMatch(match[0]),
      });
    }
  }
  return findings;
}

function listTrackedFiles() {
  const output = childProcess.execFileSync('git', ['ls-files', '-z'], {
    cwd: root,
    encoding: 'buffer',
  });
  return output.toString('utf8').split('\0').filter(Boolean);
}

function scanFiles(relativePaths) {
  const findings = [];
  for (const relativePath of relativePaths) {
    if (shouldIgnore(relativePath)) {
      continue;
    }
    const absolutePath = path.join(root, relativePath);
    const buffer = fs.readFileSync(absolutePath);
    if (isProbablyBinary(buffer)) {
      continue;
    }
    findings.push(...scanText(relativePath, buffer.toString('utf8')));
  }
  return findings;
}

function parseDateOnly(value, fieldName) {
  assert.match(value, /^\d{4}-\d{2}-\d{2}$/, `${fieldName} must be YYYY-MM-DD`);
  const parsed = new Date(`${value}T00:00:00.000Z`);
  assert.equal(parsed.toISOString().slice(0, 10), value, `${fieldName} is invalid`);
  return parsed;
}

function normalizeAllowlist(config, now = new Date()) {
  assert.equal(typeof config, 'object', 'secret scan allowlist must be an object');
  assert.notEqual(config, null, 'secret scan allowlist must be an object');

  const reviewedAt = parseDateOnly(config.reviewed_at, 'reviewed_at');
  assert.equal(
    Number.isInteger(config.review_interval_days),
    true,
    'review_interval_days must be an integer'
  );
  assert.ok(
    config.review_interval_days > 0 && config.review_interval_days <= 365,
    'review_interval_days must be between 1 and 365'
  );

  const staleAfter = new Date(
    reviewedAt.getTime() + config.review_interval_days * millisecondsPerDay
  );
  if (now.getTime() > staleAfter.getTime()) {
    throw new Error(
      `secret scan allowlist review is stale: reviewed_at ${config.reviewed_at}, interval ${config.review_interval_days} days`
    );
  }

  assert.equal(Array.isArray(config.entries), true, 'entries must be an array');
  return {
    entries: config.entries.map((entry, index) => {
      assert.equal(typeof entry.path, 'string', `entries[${index}].path must be a string`);
      assert.equal(typeof entry.type, 'string', `entries[${index}].type must be a string`);
      assert.match(
        entry.fingerprint,
        /^sha256:[a-f0-9]{64}$/,
        `entries[${index}].fingerprint must be a sha256 fingerprint`
      );
      assert.equal(typeof entry.reason, 'string', `entries[${index}].reason must be a string`);
      assert.ok(entry.reason.trim().length > 0, `entries[${index}].reason must not be empty`);
      return {
        path: entry.path,
        type: entry.type,
        fingerprint: entry.fingerprint,
      };
    }),
  };
}

function loadAllowlist() {
  const raw = fs.readFileSync(allowlistPath, 'utf8');
  return normalizeAllowlist(JSON.parse(raw));
}

function isAllowed(finding, allowlist) {
  return allowlist.entries.some(
    (entry) =>
      entry.path === finding.file &&
      entry.type === finding.type &&
      entry.fingerprint === finding.fingerprint
  );
}

function filterAllowedFindings(findings, allowlist) {
  return findings.filter((finding) => !isAllowed(finding, allowlist));
}

function runScan() {
  const allowlist = loadAllowlist();
  const findings = filterAllowedFindings(scanFiles(listTrackedFiles()), allowlist);
  if (findings.length) {
    for (const finding of findings) {
      console.error(
        `${finding.file}:${finding.line}: potential ${finding.type} (${finding.fingerprint})`
      );
    }
    throw new Error(`secret scan found ${findings.length} potential secret(s)`);
  }
  console.log('secret scan passed');
}

function runSelfTest() {
  assert.deepEqual(scanText('docs/example.md', 'export HF_TOKEN="hf_..."'), []);
  const githubFinding = scanText('leak.txt', `token=ghp_${'a'.repeat(36)}`)[0];
  assert.equal(githubFinding.type, 'github-token', 'GitHub token shape should be detected');
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
  assert.deepEqual(
    filterAllowedFindings(
      [githubFinding],
      normalizeAllowlist(
        {
          reviewed_at: '2026-06-29',
          review_interval_days: 90,
          entries: [
            {
              path: 'leak.txt',
              type: githubFinding.type,
              fingerprint: githubFinding.fingerprint,
              reason: 'self-test fake token',
            },
          ],
        },
        new Date('2026-06-30T00:00:00.000Z')
      )
    ),
    [],
    'allowlisted findings should be suppressed'
  );
  assert.throws(
    () =>
      normalizeAllowlist(
        {
          reviewed_at: '2026-01-01',
          review_interval_days: 1,
          entries: [],
        },
        new Date('2026-06-30T00:00:00.000Z')
      ),
    /stale/,
    'stale allowlist review should fail'
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
  filterAllowedFindings,
  normalizeAllowlist,
  scanText,
};
