const path = require('node:path');
const { test, expect } = require('@playwright/test');

const demoUrl = `file://${path.resolve(__dirname, '..', 'demo', 'index.html')}`;

async function openDemo(page) {
  await page.goto(demoUrl);
  await expect(page.getByText('proof market demo')).toBeVisible();
}

async function runVerification(page) {
  await page.getByTestId('run-verify').click();
  await expect(page.getByTestId('bundle')).toContainText('"magic": "VIEX"');
}

async function verdict(page) {
  return page.getByTestId('verdict').locator('b').innerText();
}

async function assertPrimaryControlsAreUsable(page) {
  const viewport = page.viewportSize();
  for (const id of ['run-verify', 'copy-bundle', 'download-bundle']) {
    const control = page.getByTestId(id);
    await expect(control).toBeVisible();
    const box = await control.boundingBox();
    expect(box, `${id} has a rendered box`).toBeTruthy();
    expect(box.width, `${id} width`).toBeGreaterThan(20);
    expect(box.height, `${id} height`).toBeGreaterThan(20);
    expect(box.x, `${id} is not clipped left`).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width, `${id} is not clipped right`).toBeLessThanOrEqual(viewport.width + 1);
  }
}

test.describe('static proof-market demo', () => {
  test('renders provider cards and usable primary controls', async ({ page }) => {
    await openDemo(page);
    await expect(page.getByTestId('provider-lab-a100-01')).toBeVisible();
    await expect(page.getByTestId('provider-desk-a10g-cheap')).toBeVisible();
    await expect(page.getByTestId('provider-frontier-proxy-x')).toBeVisible();
    await assertPrimaryControlsAreUsable(page);
  });

  test('happy path produces a passing VIEX proof bundle', async ({ page }) => {
    await openDemo(page);
    await runVerification(page);
    await expect.poll(() => verdict(page)).toBe('PASS');
    await expect(page.getByTestId('bundle')).toContainText('"overall": "pass"');
    await expect(page.getByTestId('bundle')).toContainText('"prompt_hash"');
    await expect(page.getByTestId('bundle')).toContainText('"answer_hash"');
  });

  test('release comprehension copy stays visible across happy and red paths', async ({ page }) => {
    await openDemo(page);
    await expect(page.getByText('open-weight only')).toBeVisible();
    await expect(page.getByText('simulated fixtures')).toBeVisible();
    await expect(page.getByText(/execution integrity/i)).toBeVisible();
    await expect(page.getByText(/unauthorized token resale/i)).toBeVisible();

    await runVerification(page);
    await expect.poll(() => verdict(page)).toBe('PASS');

    await page.getByTestId('mode-model_swap').click();
    await runVerification(page);
    await expect.poll(() => verdict(page)).toBe('FAIL');
    await expect(page.getByText('open-weight only')).toBeVisible();
    await expect(page.getByText('simulated fixtures')).toBeVisible();
  });

  for (const mode of ['model_swap', 'prompt_mismatch', 'answer_rewrite', 'receipt_tamper', 'expired_quote']) {
    test(`${mode} red path does not pass`, async ({ page }) => {
      await openDemo(page);
      await page.getByTestId(`mode-${mode}`).click();
      await runVerification(page);
      await expect.poll(() => verdict(page)).toBe('FAIL');
      await expect(page.getByTestId('bundle')).toContainText('"overall": "fail"');
    });
  }

  test('unsupported closed-weight provider does not pass', async ({ page }) => {
    await openDemo(page);
    await page.getByTestId('provider-frontier-proxy-x').click();
    await runVerification(page);
    await expect.poll(() => verdict(page)).toBe('UNSUPPORTED');
    await expect(page.getByTestId('bundle')).toContainText('"overall": "unsupported"');
  });
});
