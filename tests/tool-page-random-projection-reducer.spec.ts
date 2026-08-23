import { test, expect } from './fixtures';

const DATA = `ch1,ch2,ch3,ch4,ch5,ch6,ch7,ch8
1,2,3,4,5,6,7,8
8,7,6,5,4,3,2,1
2,4,6,8,10,12,14,16
0,1,0,1,0,1,0,1
5,5,5,5,5,5,5,5
9,1,8,2,7,3,6,4`;

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('random-projection-reducer page reports an exact gaussian projection', async ({ page }) => {
  await page.goto('/tools/random-projection-reducer/');
  await page.waitForSelector('#in-data');
  await expect(page.locator('#in-method')).toHaveValue('gaussian');
  await expect(page.locator('#in-format')).toHaveValue('text');
  await expect(page.locator('#in-components')).toHaveValue('');

  await setField(page, '#in-data', DATA);
  await setField(page, '#in-components', '3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Random projection: 6 rows × 8 columns → 3 dimensions', { timeout: 20_000 });
  await expect(out).toContainText('mean distortion     22.4111%');
  await expect(out).toContainText('within ±10.0000%  3 of 15 pairs (20.0000%)');
  await expect(out).toContainText('1    -17.064939     -0.160032     -8.288287');
});

test('random-projection-reducer deep link emits sparse CSV output', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    components: '3',
    method: 'sparse',
    density: '0',
    eps: '0.1',
    seed: '42',
    format: 'csv',
  });
  await page.goto(`/tools/random-projection-reducer/?${params.toString()}`);

  await expect(page.locator('#in-method')).toHaveValue('sparse', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('row,RP1,RP2,RP3', { timeout: 20_000 });
  expect(await outputText(page)).toBe(
    'row,RP1,RP2,RP3\n1,-4.854918,-4.854918,-5.825901\n2,4.854918,4.854918,-2.912951\n3,-9.709835,-9.709835,-11.651803\n4,0.970984,-0.970984,-0.970984\n5,0.000000,0.000000,-4.854918\n6,-4.854918,5.825901,0.970984',
  );
});

test('random-projection-reducer supports enum choices and boundary values', async ({ page }) => {
  await page.goto('/tools/random-projection-reducer/');
  await setField(page, '#in-data', DATA);
  await setField(page, '#in-components', '2');
  await page.selectOption('#in-method', 'rademacher');
  await setField(page, '#in-seed', '7');
  await setField(page, '#in-eps', '0.99');
  await page.selectOption('#in-format', 'matrix');

  await expect(page.locator('#tool-output')).toContainText('component,ch1,ch2,ch3,ch4,ch5,ch6,ch7,ch8', { timeout: 20_000 });
  const out = await outputText(page);
  expect(out).toContain('RP1,0.707107,-0.707107,-0.707107,-0.707107,-0.707107,0.707107,-0.707107,-0.707107');
  expect(out).toContain('RP2,0.707107,0.707107,0.707107,-0.707107,0.707107,0.707107,0.707107,0.707107');

  await page.selectOption('#in-method', 'achlioptas');
  await page.selectOption('#in-format', 'text');
  await setField(page, '#in-density', '1');
  await expect(page.locator('#tool-output')).toContainText('method        achlioptas', { timeout: 20_000 });
  await expect(page.locator('#tool-output')).toContainText('density       100.0000% non-zero entries');
});

test('random-projection-reducer page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/random-projection-reducer/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool random-projection-reducer');
  expect(cli).toContain('ch1,ch2,ch3,ch4,ch5,ch6,ch7,ch8');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
  await expect(page.locator('#in-method option[value="sparse"]')).toHaveText('Sparse — ±values at density 1/√columns, zeros elsewhere');
  await expect(page.locator('#in-format option[value="matrix"]')).toHaveText('CSV (the projection matrix itself)');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
});
