const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const reportPath = path.join(root, 'reports', 'perf', 'size-budgets.json');

const budgets = [
  {
    label: 'Static demo HTML',
    path: 'demo/index.html',
    limit_bytes: 500 * 1024,
    source: 'docs/spec/08-performance-budget.md',
  },
  {
    label: 'Browser verifier WASM',
    path: 'verifier/wasm/pkg/vi_commitllm_verifier_bg.wasm',
    limit_bytes: 10 * 1024 * 1024,
    source: 'docs/spec/08-performance-budget.md',
  },
  {
    label: 'WASM JS loader',
    path: 'verifier/wasm/pkg/vi_commitllm_verifier.js',
    limit_bytes: 500 * 1024,
    source: 'local CI budget',
  },
];

function fileSizeBytes(relativePath) {
  return fs.statSync(path.join(root, relativePath)).size;
}

function collectViexBudgets() {
  const fixtureDirectory = path.join(root, 'fixtures', 'viex');
  return fs
    .readdirSync(fixtureDirectory)
    .filter((file) => file.endsWith('.json') && file !== 'manifest.json')
    .sort()
    .map((file) => ({
      label: `VIEX fixture ${file}`,
      path: path.join('fixtures', 'viex', file),
      limit_bytes: 250 * 1024,
      source: 'docs/spec/08-performance-budget.md',
    }));
}

function collectRows() {
  return [...budgets, ...collectViexBudgets()].map((budget) => {
    const size_bytes = fileSizeBytes(budget.path);
    return {
      ...budget,
      size_bytes,
      within_budget: size_bytes <= budget.limit_bytes,
    };
  });
}

function renderMarkdown(rows) {
  const lines = [
    '## Size Budgets',
    '',
    '| Artifact | Size | Limit | Status |',
    '| --- | ---: | ---: | --- |',
  ];
  for (const row of rows) {
    lines.push(
      `| \`${row.path}\` | ${row.size_bytes} bytes | ${row.limit_bytes} bytes | ${
        row.within_budget ? 'pass' : 'fail'
      } |`
    );
  }
  lines.push('');
  return `${lines.join('\n')}\n`;
}

function writeReport(rows) {
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(
    reportPath,
    `${JSON.stringify(
      {
        generated_at: new Date().toISOString(),
        budgets: rows,
      },
      null,
      2
    )}\n`
  );
}

function writeSummary(rows) {
  const summaryPath = process.env.GITHUB_STEP_SUMMARY;
  if (!summaryPath) {
    return;
  }
  fs.appendFileSync(summaryPath, renderMarkdown(rows));
}

function run() {
  const rows = collectRows();
  writeReport(rows);
  writeSummary(rows);

  const failures = rows.filter((row) => !row.within_budget);
  if (failures.length) {
    for (const failure of failures) {
      console.error(
        `${failure.path}: ${failure.size_bytes} bytes exceeds ${failure.limit_bytes} byte budget`
      );
    }
    throw new Error(`size budget failed for ${failures.length} artifact(s)`);
  }

  console.log(renderMarkdown(rows).trim());
}

if (require.main === module) {
  try {
    run();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}

module.exports = {
  collectRows,
  renderMarkdown,
};
