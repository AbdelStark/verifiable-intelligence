const fs = require('node:fs/promises');
const path = require('node:path');
const { test, expect } = require('@playwright/test');

const harnessRoot = path.resolve(__dirname, '..', 'verifier', 'wasm');
const harnessOrigin = 'http://127.0.0.1:41731';

const contentTypes = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.bin': 'application/octet-stream'
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
        'content-type': contentTypes[path.extname(filePath)] || 'application/octet-stream',
        'cache-control': 'no-store'
      }
    });
  });
}

test.describe('CommitLLM browser WASM verifier', () => {
  test('verifies a VIEX bundle receipt and rejects a tampered receipt in browser', async ({ page }) => {
    await routeHarness(page);
    await page.goto(`${harnessOrigin}/harness.html`);
    await expect(page.locator('#status')).toHaveText('pass');

    const results = await page.evaluate(() => window.__commitllmWasmResults);
    expect(results.package.commitllm_pin).toBe('25541e83347655e44ad6e84eb901e1e7ae392a66');
    expect(results.package.verification_mode).toBe('browser-wasm');
    expect(results.happy.last.overall).toBe('pass');
    expect(results.happy.last.commitllm.overall).toBe('pass');
    expect(results.happy.last.commitllm.checks_run).toBeGreaterThan(0);
    expect(results.tampered.last.overall).toBe('fail');
    expect(results.tampered.last.commitllm.overall).toBe('fail');
    expect(results.tampered.last.commitllm.error).toMatch(/zstd|deserialization|decode/i);
    expect(results.happy.p50_ms).toBeGreaterThanOrEqual(0);
    expect(results.tampered.p50_ms).toBeGreaterThanOrEqual(0);
  });
});
