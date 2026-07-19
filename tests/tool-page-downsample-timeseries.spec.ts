import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

// 20-point fixture with two spikes (45 @ t=5, 80 @ t=17) — expected selections
// cross-checked against an independent Python port of the canonical
// flot-downsample LTTB.
const DATA =
  't,v\n1,10\n2,12\n3,11\n4,13\n5,45\n6,12\n7,10\n8,11\n9,13\n10,12\n11,14\n12,13\n13,12\n14,15\n15,13\n16,12\n17,80\n18,13\n19,12\n20,11';
const LTTB_8 = 't,v\n1,10\n4,13\n5,45\n8,11\n11,14\n16,12\n17,80\n20,11';

test('lttb keeps spikes and endpoints (exact multi-line output)', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '8');
  await page.fill('#in-data', DATA);
  await expect.poll(() => output(page), { timeout: 15000 }).toBe(LTTB_8);
});

test('minmax algorithm with indices output', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '8');
  await page.selectOption('#in-algorithm', 'minmax');
  await page.selectOption('#in-output', 'indices');
  await page.fill('#in-data', DATA);
  await expect(page.locator('#tool-output')).toHaveText('[0,4,6,8,12,13,16,19]', { timeout: 15000 });
});

test('m4 algorithm with indices output', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '8');
  await page.selectOption('#in-algorithm', 'm4');
  await page.selectOption('#in-output', 'indices');
  await page.fill('#in-data', DATA);
  await expect(page.locator('#tool-output')).toHaveText('[0,4,9,10,16,19]', { timeout: 15000 });
});

test('nth algorithm with indices output', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '8');
  await page.selectOption('#in-algorithm', 'nth');
  await page.selectOption('#in-output', 'indices');
  await page.fill('#in-data', DATA);
  await expect(page.locator('#tool-output')).toHaveText('[0,3,5,8,11,14,16,19]', { timeout: 15000 });
});

test('json pairs input (secondary input format) returns original elements', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '3');
  await page.fill('#in-data', '[[1,10],[2,45],[3,12],[4,11],[5,13],[6,9]]');
  await expect.poll(() => output(page), { timeout: 15000 }).toBe('[\n  [1,10],\n  [2,45],\n  [6,9]\n]');
});

test('iso date x values with y_column by header name', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '4');
  await page.fill('#in-y_column', 'close');
  await page.fill(
    '#in-data',
    'date,close\n2024-01-01,10\n2024-01-02,12\n2024-01-03,11\n2024-01-05,45\n2024-01-08,12\n2024-01-09,10'
  );
  await expect
    .poll(() => output(page), { timeout: 15000 })
    .toBe('date,close\n2024-01-01,10\n2024-01-03,11\n2024-01-05,45\n2024-01-09,10');
});

test('non-default header checkbox: unchecked makes a text first row an error', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '2');
  await page.uncheck('#in-header');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await page.fill('#in-data', 't,v\n1,5\n2,6');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("line 1: 'v' is not a finite number", { timeout: 15000 });
  await expect(out).toHaveClass(/error/);
});

test('points minimum boundary: 2 keeps first and last, 1 is rejected', async ({ page }) => {
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '2');
  await page.fill('#in-data', DATA);
  await expect.poll(() => output(page), { timeout: 15000 }).toBe('t,v\n1,10\n20,11');
  await page.fill('#in-points', '1');
  await expect(page.locator('#tool-output')).toContainText('between 2 and 100000', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveClass(/error/);
});

test('byte cap boundary: exactly 2,000,000 bytes accepted, one over rejected', async ({ page }) => {
  test.setTimeout(120000);
  await page.goto('/tools/downsample-timeseries/');
  await page.fill('#in-points', '2');
  await page.selectOption('#in-algorithm', 'nth');
  await page.selectOption('#in-output', 'indices');
  // Big fixture: set the value directly + dispatch 'input' (page.fill on huge
  // textareas routes through insertText and takes minutes — see page-patterns).
  const setData = (v: string) =>
    page.locator('#in-data').evaluate((el: HTMLTextAreaElement, val: string) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, v);
  let at = '5\n6\n7\n1.';
  at += '0'.repeat(2000000 - at.length);
  expect(at.length).toBe(2000000);
  await setData(at);
  await expect(page.locator('#tool-output')).toHaveText('[0,3]', { timeout: 30000 });
  await setData(at + '0');
  await expect(page.locator('#tool-output')).toContainText('cap is 2000000 bytes', { timeout: 30000 });
  await expect(page.locator('#tool-output')).toHaveClass(/error/);
});

test('deep-link pre-fills params and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    points: '8',
    algorithm: 'nth',
    output: 'indices',
  });
  await page.goto(`/tools/downsample-timeseries/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(DATA, { timeout: 15000 });
  await expect(page.locator('#in-algorithm')).toHaveValue('nth');
  await expect(page.locator('#tool-output')).toHaveText('[0,3,5,8,11,14,16,19]', { timeout: 15000 });
});
