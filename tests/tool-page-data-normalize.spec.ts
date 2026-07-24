import { test, expect } from './fixtures';

const minMaxCsv = 'name,height,weight\nAlice,150,50\nBob,160,60\nCarol,170,70';

test('data-normalize page min-max scales numeric columns to 0-1', async ({ page }) => {
  await page.goto('/tools/data-normalize/');
  await page.fill('#in-input', minMaxCsv);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Bob,0.5,0.5', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'name,height,weight\nAlice,0,0\nBob,0.5,0.5\nCarol,1,1\n',
  );
});

test('data-normalize deep link covers non-default method, ddof, and checkbox controls', async ({ page }) => {
  const csv = 'v\n1\n2\n3\n4\n5';
  const qs =
    '?input=' + encodeURIComponent(csv) +
    '&method=robust' +
    '&header=true' +
    '&delimiter=comma' +
    '&columns=v' +
    '&with_centering=false' +
    '&with_scaling=true';
  await page.goto('/tools/data-normalize/' + qs);

  await expect(page.locator('#in-method')).toHaveValue('robust', { timeout: 15_000 });
  await expect(page.locator('#in-columns')).toHaveValue('v');
  await expect(page.locator('#in-with_centering')).not.toBeChecked();
  await expect(page.locator('#in-with_scaling')).toBeChecked();

  // No centering, IQR = 2 → each value divided by 2.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('0.5', { timeout: 15_000 });
  expect(await out.textContent()).toBe('v\n0.5\n1\n1.5\n2\n2.5\n');
});

test('data-normalize page z-score standardizes with sample stddev', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('score\n1\n2\n3') +
    '&method=z_score' +
    '&ddof=1';
  await page.goto('/tools/data-normalize/' + qs);

  await expect(page.locator('#in-method')).toHaveValue('z_score', { timeout: 15_000 });
  await expect(page.locator('#in-ddof')).toHaveValue('1');

  // mean 2, sample stddev 1 → -1, 0, 1.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('-1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('score\n-1\n0\n1\n');
});
