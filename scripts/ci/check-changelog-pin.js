const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const rfcLink =
  'https://github.com/AbdelStark/verifiable-intelligence/blob/main/docs/rfcs/RFC-0011-commitllm-upstream-pinning.md#pin-change-checklist-binding-rule';

function runGit(args, options = {}) {
  return childProcess.execFileSync('git', args, {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', options.allowFailure ? 'ignore' : 'pipe'],
  });
}

function tryGit(args) {
  try {
    return runGit(args).trim();
  } catch (_error) {
    return null;
  }
}

function resolveBaseRef() {
  const baseRef = process.env.GITHUB_BASE_REF;
  if (baseRef) {
    tryGit(['fetch', '--no-tags', '--depth=1', 'origin', `${baseRef}:refs/remotes/origin/${baseRef}`]);
    return `origin/${baseRef}`;
  }

  if (tryGit(['rev-parse', '--verify', 'HEAD^'])) {
    return 'HEAD^';
  }

  return null;
}

function changedFiles(baseRef) {
  if (!baseRef) {
    return [];
  }
  const diffBase = baseRef.includes('...') ? baseRef : `${baseRef}...HEAD`;
  const output = tryGit(['diff', '--name-only', diffBase]);
  if (!output) {
    return [];
  }
  return output.split(/\r?\n/).filter(Boolean);
}

function fileAtRef(ref, relativePath) {
  if (!ref) {
    return '';
  }
  try {
    return runGit(['show', `${ref}:${relativePath}`]);
  } catch (_error) {
    return '';
  }
}

function currentFile(relativePath) {
  const absolutePath = path.join(root, relativePath);
  if (!fs.existsSync(absolutePath)) {
    return '';
  }
  return fs.readFileSync(absolutePath, 'utf8');
}

function section(lines, headingPattern, stopPattern, start = 0) {
  const headingIndex = lines.findIndex((line, index) => index >= start && headingPattern.test(line));
  if (headingIndex === -1) {
    return null;
  }
  const contentStart = headingIndex + 1;
  const relativeEnd = lines
    .slice(contentStart)
    .findIndex((line) => stopPattern.test(line));
  const contentEnd = relativeEnd === -1 ? lines.length : contentStart + relativeEnd;
  return { start: contentStart, end: contentEnd };
}

function pinEntries(markdown) {
  const lines = markdown.split(/\r?\n/);
  const unreleased = section(lines, /^## \[Unreleased\]\s*$/, /^## /);
  if (!unreleased) {
    return [];
  }

  const pin = section(
    lines,
    /^### Pin\s*$/,
    /^### /,
    unreleased.start
  );
  if (!pin || pin.start > unreleased.end) {
    return [];
  }

  const pinEnd = Math.min(pin.end, unreleased.end);
  return lines
    .slice(pin.start, pinEnd)
    .map((line) => line.trim())
    .filter((line) => /^-\s+\S/.test(line));
}

function evaluate({ filesChanged, baseChangelog, currentChangelog }) {
  if (!filesChanged.includes('commitllm.lock')) {
    return {
      ok: true,
      message: 'commitllm.lock did not change; CHANGELOG Pin entry not required',
    };
  }

  if (!currentChangelog.trim()) {
    return {
      ok: false,
      message: `commitllm.lock changed, but CHANGELOG.md is missing. Add an entry under ## [Unreleased] -> ### Pin. See ${rfcLink}`,
    };
  }

  const before = new Set(pinEntries(baseChangelog));
  const added = pinEntries(currentChangelog).filter((entry) => !before.has(entry));
  if (added.length === 0) {
    return {
      ok: false,
      message: `commitllm.lock changed, but this PR did not add a new CHANGELOG.md entry under ## [Unreleased] -> ### Pin. See ${rfcLink}`,
    };
  }

  return {
    ok: true,
    message: `CHANGELOG Pin entry found: ${added[0]}`,
  };
}

function runCiCheck() {
  const baseRef = resolveBaseRef();
  const filesChanged = changedFiles(baseRef);
  const result = evaluate({
    filesChanged,
    baseChangelog: fileAtRef(baseRef, 'CHANGELOG.md'),
    currentChangelog: currentFile('CHANGELOG.md'),
  });

  if (!result.ok) {
    throw new Error(result.message);
  }

  console.log(result.message);
}

function runSelfTest() {
  const emptyChangelog = '';
  const scaffold = [
    '# Changelog',
    '',
    '## [Unreleased]',
    '',
    '### Added',
    '',
    '### Pin',
    '',
    '',
  ].join('\n');
  const withPin = [
    '# Changelog',
    '',
    '## [Unreleased]',
    '',
    '### Pin',
    '',
    '- CommitLLM pin: old -> new; regenerated fixtures.',
    '',
    '### Fixed',
    '',
  ].join('\n');
  const withTwoPins = withPin.replace(
    '### Fixed',
    '- CommitLLM pin: new -> newer; upstream rename.\n\n### Fixed'
  );

  assert.equal(
    evaluate({
      filesChanged: ['README.md'],
      baseChangelog: emptyChangelog,
      currentChangelog: emptyChangelog,
    }).ok,
    true,
    'non-pin changes do not require changelog Pin entries'
  );
  assert.equal(
    evaluate({
      filesChanged: ['commitllm.lock'],
      baseChangelog: emptyChangelog,
      currentChangelog: emptyChangelog,
    }).ok,
    false,
    'pin change without CHANGELOG.md fails'
  );
  assert.equal(
    evaluate({
      filesChanged: ['commitllm.lock'],
      baseChangelog: emptyChangelog,
      currentChangelog: scaffold,
    }).ok,
    false,
    'pin change without Pin bullet fails'
  );
  assert.equal(
    evaluate({
      filesChanged: ['commitllm.lock'],
      baseChangelog: emptyChangelog,
      currentChangelog: withPin,
    }).ok,
    true,
    'pin change with new Pin bullet passes'
  );
  assert.equal(
    evaluate({
      filesChanged: ['commitllm.lock'],
      baseChangelog: withPin,
      currentChangelog: withPin,
    }).ok,
    false,
    'pin change with only existing Pin bullet fails'
  );
  assert.equal(
    evaluate({
      filesChanged: ['commitllm.lock'],
      baseChangelog: withPin,
      currentChangelog: withTwoPins,
    }).ok,
    true,
    'pin change with an added Pin bullet passes'
  );

  console.log('CHANGELOG pin lint self-test passed');
}

if (require.main === module) {
  if (process.argv.includes('--self-test')) {
    runSelfTest();
  } else {
    runCiCheck();
  }
}

module.exports = {
  evaluate,
  pinEntries,
};

