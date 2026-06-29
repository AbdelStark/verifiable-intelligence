const assert = require('node:assert/strict');

const upstreamOwner = 'lambdaclass';
const upstreamRepo = 'CommitLLM';
const issueTitle = 'CommitLLM rename merged: bump pin';
const apiUrl = process.env.GITHUB_API_URL || 'https://api.github.com';

function detectRename(crateNames) {
  return crateNames.some((name) => /^commitllm[-_]/i.test(name));
}

function issueBody(crateNames) {
  return [
    'The upstream CommitLLM rename appears to have landed on `lambdaclass/CommitLLM@main`.',
    '',
    'Observed crate markers:',
    '',
    ...crateNames.map((name) => `- \`${name}\``),
    '',
    'Next steps:',
    '',
    '- Follow RFC-0011 pin change checklist.',
    '- Update `commitllm.lock`, Cargo dependencies, Dockerfile pin, fixtures, README pin text, and `CHANGELOG.md` under `## [Unreleased]` -> `### Pin`.',
    '- Run the full pin-bump validation before merging.',
  ].join('\n');
}

async function requestJson(path, options = {}) {
  const token = process.env.GITHUB_TOKEN;
  const headers = {
    accept: 'application/vnd.github+json',
    'user-agent': 'verifiable-intelligence-commitllm-rename-watch',
    ...options.headers,
  };
  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  const response = await fetch(`${apiUrl}${path}`, {
    ...options,
    headers,
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`GitHub API ${options.method || 'GET'} ${path} failed: ${response.status} ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function upstreamCrateNames() {
  const entries = await requestJson(
    `/repos/${upstreamOwner}/${upstreamRepo}/contents/crates?ref=main`
  );
  if (!Array.isArray(entries)) {
    throw new Error('unexpected GitHub API response for upstream crates directory');
  }
  return entries.map((entry) => entry.name).sort();
}

async function existingOpenIssue(repository) {
  const query = encodeURIComponent(`repo:${repository} is:issue is:open in:title "${issueTitle}"`);
  const result = await requestJson(`/search/issues?q=${query}`);
  return (result.items || []).find((item) => item.title === issueTitle) || null;
}

async function openBumpIssue(repository, crateNames) {
  if (!process.env.GITHUB_TOKEN) {
    throw new Error('GITHUB_TOKEN is required to open the CommitLLM pin-bump issue');
  }
  return requestJson(`/repos/${repository}/issues`, {
    method: 'POST',
    body: JSON.stringify({
      title: issueTitle,
      body: issueBody(crateNames),
    }),
  });
}

async function run() {
  const repository = process.env.GITHUB_REPOSITORY;
  if (!repository) {
    throw new Error('GITHUB_REPOSITORY is required');
  }

  const crateNames = await upstreamCrateNames();
  if (!detectRename(crateNames)) {
    console.log(`CommitLLM rename not detected; upstream crates: ${crateNames.join(', ')}`);
    return;
  }

  const existing = await existingOpenIssue(repository);
  if (existing) {
    console.log(`CommitLLM rename detected; existing issue already open: ${existing.html_url}`);
    return;
  }

  const issue = await openBumpIssue(repository, crateNames);
  console.log(`CommitLLM rename detected; opened issue: ${issue.html_url}`);
}

function runSelfTest() {
  assert.equal(detectRename(['verilm-core', 'verilm-verify']), false);
  assert.equal(detectRename(['verilm-core', 'commitllm-verify']), true);
  assert.equal(detectRename(['commitllm_core']), true);
  assert.match(issueBody(['commitllm-core']), /RFC-0011 pin change checklist/);
  assert.match(issueBody(['commitllm-core']), /CHANGELOG\.md/);
  console.log('CommitLLM rename watcher self-test passed');
}

if (require.main === module) {
  if (process.argv.includes('--self-test')) {
    runSelfTest();
  } else {
    run().catch((error) => {
      console.error(error.message);
      process.exit(1);
    });
  }
}

module.exports = {
  detectRename,
  issueBody,
};

