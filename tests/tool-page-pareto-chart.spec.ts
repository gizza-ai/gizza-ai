import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setData(page: any, value: string) {
  await page.locator('#in-data').evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('pareto-chart page renders real SVG with bars and cumulative line', async ({ page }) => {
  await page.goto('/tools/pareto-chart/');
  await setData(page, 'Reason,Count\nLate delivery,45\nWrong item,30\nDamaged,15\nBilling error,7\nRude staff,3');
  await page.fill('#in-title', 'Q3 customer complaints');
  await page.check('#in-show_values');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Q3 customer complaints');
  expect(text).toContain('Late delivery');
  expect(text).toContain('Wrong item');
  expect(text).toContain('stroke="#dc2626"');
  expect(text).toContain('80% threshold');
});

test('pareto-chart page deep-link renders tail bucket summary', async ({ page }) => {
  const data = 'A,50\nB,30\nC,15\nD,7\nE,4\nF,2';
  await page.goto(
    '/tools/pareto-chart/?data=' +
      encodeURIComponent(data) +
      '&max_categories=3&other_label=All%20other%20causes&output=summary',
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#tool-output')).toContainText('All other causes', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('TOTAL');
  expect(text).toContain('Vital few:');
});

test('pareto-chart page advertised controls handle pipe input and dark SVG', async ({ page }) => {
  await page.goto('/tools/pareto-chart/');
  await setData(page, 'Bug type|Count\nCrash|120\nUI|45\nDocs|20\nPerformance|15');
  await page.selectOption('#in-delimiter', 'pipe');
  await page.selectOption('#in-header', 'auto');
  await page.selectOption('#in-sort', 'desc');
  await page.selectOption('#in-theme', 'dark');
  await page.selectOption('#in-output', 'svg');
  await page.fill('#in-threshold', '75');
  await page.fill('#in-color', '#38bdf8');
  await page.fill('#in-vital_color', '#f97316');
  await page.fill('#in-line_color', '#f00');
  await page.fill('#in-threshold_color', '#94a3b8');
  await page.fill('#in-label_angle', '30');
  await page.fill('#in-bar_width', '1');
  await page.fill('#in-line_width', '3');
  await page.fill('#in-point_radius', '4');
  await page.check('#in-show_cumulative_labels');
  await page.uncheck('#in-grid');
  await page.uncheck('#in-legend');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('fill="#0f172a"');
  expect(text).toContain('fill="#f97316"');
  expect(text).toContain('stroke="#f00"');
  expect(text).toContain('75% threshold');
});

test('pareto-chart page cap boundary and invalid row error', async ({ page }) => {
  await page.goto('/tools/pareto-chart/');
  const rows = Array.from({ length: 500 }, (_, i) => `R${i + 1},1`).join('\n');
  await setData(page, rows);
  await page.selectOption('#in-output', 'summary');
  await expect(page.locator('#tool-output')).toContainText('TOTAL', { timeout: 15000 });

  await setData(page, 'Good,1\nBad,not-a-number');
  await expect(page.locator('#tool-output')).toContainText('expected', { timeout: 15000 });
});
