import { test, expect } from './fixtures';

const tool = '/tools/correlated-sample-generator/';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('correlated-sample-generator page emits deterministic CSV rows', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-covariance', '1, 0.8; 0.8, 1');
  await page.fill('#in-samples', '3');
  await page.fill('#in-seed', '42');
  await page.fill('#in-decimals', '3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('X1,X2', { timeout: 15000 });
  await expect(out).toContainText('-0.342,0.578');
  await expect(out).toContainText('0.105,-0.130');
  await expect(out).toContainText('0.274,-0.492');
});

test('correlated-sample-generator page supports correlation mode and JSON statistics', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-covariance', '1, 0.5; 0.5, 1');
  await page.selectOption('#in-matrix_kind', 'correlation');
  await page.fill('#in-sd', '2, 3');
  await page.fill('#in-mean', '10, -4');
  await page.fill('#in-samples', '5');
  await page.selectOption('#in-output', 'json');
  await page.fill('#in-decimals', '2');
  await page.fill('#in-labels', 'height, weight');

  const parsed = JSON.parse(await outputText(page));
  expect(parsed.count).toBe(5);
  expect(parsed.dimensions).toBe(2);
  expect(parsed.labels).toEqual(['height', 'weight']);
  expect(parsed.target.covariance[0][1]).toBe(3);
  expect(parsed.samples[0]).toHaveLength(2);
});

test('correlated-sample-generator page handles eigen method and header checkbox', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-covariance', '1, 1; 1, 1');
  await page.selectOption('#in-method', 'eigen');
  await page.fill('#in-samples', '4');
  await page.fill('#in-seed', '1');
  await page.selectOption('#in-output', 'stats');
  await page.fill('#in-decimals', '3');
  await page.uncheck('#in-header');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('4 draw(s) of 2 variable(s) — method eigen, seed 1', { timeout: 15000 });
  await expect(out).toContainText('Target covariance:');
  await expect(out).toContainText('Sample correlation:');
  await expect(out).toContainText('X1  1.000  1.000');
});

test('correlated-sample-generator query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?covariance=' +
      encodeURIComponent('ar1(3, 0.5)') +
      '&matrix_kind=covariance&samples=4&method=cholesky&seed=5&output=csv&decimals=2&labels=a%2Cb%2Cc&header=false&tol=0.00000001',
  );

  await expect(page.locator('#in-covariance')).toHaveValue('ar1(3, 0.5)', { timeout: 15000 });
  await expect(page.locator('#in-labels')).toHaveValue('a,b,c');
  await expect(page.locator('#in-header')).not.toBeChecked();
  const out = await outputText(page);
  expect(out).not.toContain('a,b,c');
  expect(out).toContain('0.80,0.20,1.75');
});
