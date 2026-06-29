const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('@playwright/test');

const root = path.resolve(__dirname, '..', '..');
const harnessRoot = path.join(root, 'verifier', 'wasm');
const harnessOrigin = 'http://127.0.0.1:41731';
const reportPath = path.join(root, 'reports', 'perf', 'browser-verifier.json');

const contentTypes = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.bin': 'application/octet-stream',
};

async function routeHarness(page) {
  await page.route(`${harnessOrigin}/**`, async (route) => {
    const url = new URL(route.request().url());
    const relativePath = decodeURIComponent(url.pathname === '/' ? '/harness.html' : url.pathname);
    const filePath = path.resolve(harnessRoot, `.${relativePath}`);
    const rootRelativePath = path.relative(harnessRoot, filePath);
    if (rootRelativePath.startsWith('..') || path.isAbsolute(rootRelativePath)) {
      await route.abort();
      return;
    }
    const body = await fs.readFile(filePath);
    await route.fulfill({
      status: 200,
      body,
      headers: {
        'cache-control': 'no-store',
        'content-type': contentTypes[path.extname(filePath)] || 'application/octet-stream',
      },
    });
  });
}

async function fileSize(relativePath) {
  const stat = await fs.stat(path.join(root, relativePath));
  return stat.size;
}

function compactRun(run) {
  return {
    sample_count: run.iterations,
    min_ms: run.min_ms,
    p50_ms: run.p50_ms,
    p95_ms: run.p95_ms,
    max_ms: run.max_ms,
    last_overall: run.last.overall,
    commitllm_overall: run.last.commitllm.overall,
  };
}

async function collectReport() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await routeHarness(page);
    await page.goto(`${harnessOrigin}/harness.html`);
    await page.waitForFunction(() => window.__commitllmWasmResults?.happy?.last);

    const status = await page.locator('#status').textContent();
    if (status !== 'pass') {
      throw new Error(`browser verifier harness status was ${status}`);
    }

    const results = await page.evaluate(() => window.__commitllmWasmResults);
    if (results.happy.last.overall !== 'pass') {
      throw new Error('happy-path browser verifier result did not pass');
    }
    if (results.tampered.last.overall !== 'fail') {
      throw new Error('tampered browser verifier result did not fail');
    }

    return {
      generated_at: new Date().toISOString(),
      package: results.package,
      browser: {
        name: 'chromium',
        headless: true,
      },
      runs: {
        happy: compactRun(results.happy),
        tampered: compactRun(results.tampered),
      },
      memory: results.memory,
      artifacts: {
        wasm_bytes: await fileSize('verifier/wasm/pkg/vi_commitllm_verifier_bg.wasm'),
        loader_bytes: await fileSize('verifier/wasm/pkg/vi_commitllm_verifier.js'),
        viex_fixture_bytes: await fileSize(
          'verifier/wasm/fixtures/commitllm-fullbridge.viex.json'
        ),
        key_fixture_bytes: await fileSize('verifier/wasm/fixtures/v4_key_fullbridge.bin'),
        audit_fixture_bytes: await fileSize('verifier/wasm/fixtures/v4_audit_fullbridge.bin'),
      },
    };
  } finally {
    await browser.close();
  }
}

async function run() {
  const report = await collectReport();
  await fs.mkdir(path.dirname(reportPath), { recursive: true });
  await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`wrote ${path.relative(root, reportPath)}`);
  console.log(
    `happy p50=${report.runs.happy.p50_ms}ms p95=${report.runs.happy.p95_ms}ms; ` +
      `tampered p50=${report.runs.tampered.p50_ms}ms p95=${report.runs.tampered.p95_ms}ms`
  );
}

if (require.main === module) {
  run().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}

module.exports = {
  collectReport,
};
