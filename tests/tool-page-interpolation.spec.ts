import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('linear interpolation renders exact values', async ({ page }) => {
  await page.goto('/tools/interpolation/');
  await setTextarea(page, '#in-data', '0,0\n10,100');
  await page.fill('#in-at', '2.5, 5, 7.5');
  await page.fill('#in-decimals', '2');

  await expect(page.locator('#tool-output')).toContainText('2.5,25', { timeout: 15000 });
  expect((await outText(page)).trimEnd()).toBe('2.5,25\n5,50\n7.5,75');
});

test('deep-link pre-fills and auto-runs cubic not-a-knot with coefficients', async ({ page }) => {
  const data = '0,0\n1,1\n2,8\n3,27\n4,64';
  await page.goto(
    '/tools/interpolation/?data=' +
      encodeURIComponent(data) +
      '&method=cubic&boundary=not-a-knot&at=1.5%2C%202.5&coefficients=true&output=values',
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-method')).toHaveValue('cubic');
  await expect(page.locator('#in-boundary')).toHaveValue('not-a-knot');
  await expect(page.locator('#in-coefficients')).toBeChecked();
  const text = await outText(page);
  expect(text).toContain('1.5,3.375');
  expect(text).toContain('2.5,15.625');
  expect(text).toContain('[1, 2]  y = 1 + 3(x - 1) + 3(x - 1)^2 + (x - 1)^3');
});

test('monotone resample CSV exercises enum, slider and CSV output', async ({ page }) => {
  await page.goto('/tools/interpolation/');
  await setTextarea(page, '#in-data', '0,0\n1,0\n2,0\n3,1\n4,1\n5,1');
  await page.selectOption('#in-method', 'monotone');
  await page.fill('#in-resample', '6');
  await page.fill('#in-decimals', '4');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('x,y,source,extrapolated', { timeout: 15000 });
  expect((await outText(page)).trimEnd()).toBe('x,y,source,extrapolated\n0,0,resample,false\n1,0,resample,false\n2,0,resample,false\n3,1,resample,false\n4,1,resample,false\n5,1,resample,false');
});

test('polynomial JSON output and extrapolation warning parse as structured data', async ({ page }) => {
  await page.goto('/tools/interpolation/');
  await setTextarea(page, '#in-data', '1,1\n2,4\n3,9');
  await page.selectOption('#in-method', 'polynomial');
  await page.fill('#in-at', '4');
  await page.selectOption('#in-extrapolate', 'extend');
  await page.check('#in-coefficients');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toContainText('"method": "polynomial"', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.evaluations[0]).toMatchObject({ x: 4, value: 16, source: 'at', extrapolated: true });
  expect(parsed.polynomial.equation).toBe('y = x^2');
  expect(parsed.warnings[0]).toContain('were extrapolated');
});

test('nearest, derivatives, SVG, and cap boundary are advertised values', async ({ page }) => {
  await page.goto('/tools/interpolation/');
  await setTextarea(page, '#in-data', '1,10\n2,20');
  await page.selectOption('#in-method', 'nearest');
  await page.fill('#in-at', '1.5');
  await expect(page.locator('#tool-output')).toContainText('1.5,20', { timeout: 15000 });

  await page.selectOption('#in-method', 'linear');
  await page.fill('#in-derivative', '1');
  await expect(page.locator('#tool-output')).toContainText('1.5,10', { timeout: 15000 });

  await page.fill('#in-derivative', '0');
  await page.selectOption('#in-output', 'svg');
  await expect(page.locator('#tool-output')).toContainText('<svg xmlns="http://www.w3.org/2000/svg"', { timeout: 15000 });

  await page.selectOption('#in-output', 'values');
  await page.fill('#in-resample', '5000');
  await expect(page.locator('#tool-output')).toContainText('2,20', { timeout: 15000 });
  await page.fill('#in-resample', '5001');
  await expect(page.locator('#tool-output')).toContainText('resample must be at most 5000', { timeout: 15000 });
});
