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

test('treemap-chart page renders real SVG from flat rows', async ({ page }) => {
  await page.goto('/tools/treemap-chart/');
  await setData(page, 'Documents,4200\nPhotos,3100\nVideos,2400\nMusic,900');
  await page.fill('#in-title', 'Storage by folder');
  await page.check('#in-show_percent');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Storage by folder');
  expect(text).toContain('Documents');
  expect(text).toContain('4,200');
  expect(text).toContain('39.6%');
  expect(text).toContain('fill="#2563eb"');
});

test('treemap-chart page deep-link renders grouped dark mono summary', async ({ page }) => {
  const data = 'region,city,people\nEU,Paris,2100\nEU,Rome,2800\nUS,Austin,960';
  await page.goto(
    '/tools/treemap-chart/?data=' +
      encodeURIComponent(data) +
      '&layout=grouped&output=summary&theme=dark&palette=mono&color=%2322d3ee&show_values=false&show_percent=true',
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-layout')).toHaveValue('grouped');
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#tool-output')).toContainText('EU / Paris', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('layout: grouped');
  expect(text).toContain('tiles: 3');
  expect(text).toContain('total: 5,860');
});

test('treemap-chart page advertised enum values and short hex render', async ({ page }) => {
  await page.goto('/tools/treemap-chart/');
  await setData(page, 'A,50\nB,30\nC,20');
  await page.selectOption('#in-layout', 'flat');
  await page.selectOption('#in-sort', 'value_asc');
  await page.selectOption('#in-tiling', 'binary');
  await page.selectOption('#in-palette', 'mono');
  await page.fill('#in-color', '#f00');
  await page.selectOption('#in-label_position', 'center');
  await page.selectOption('#in-theme', 'light');
  await page.selectOption('#in-output', 'svg');
  await page.uncheck('#in-show_values');
  await page.check('#in-show_percent');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('fill="#f00"');
  expect(text).toContain('text-anchor="middle"');
  expect(text).toContain('50.0%');
});

test('treemap-chart page cap boundary and invalid value error', async ({ page }) => {
  await page.goto('/tools/treemap-chart/');
  const rows = Array.from({ length: 20000 }, (_, i) => `R${i + 1},1`).join('\n');
  await setData(page, rows);
  await page.selectOption('#in-output', 'summary');
  await page.fill('#in-top_n', '500');
  await expect(page.locator('#tool-output')).toContainText('total: 20,000', { timeout: 15000 });

  await setData(page, 'Good,1\nBad,lots');
  await expect(page.locator('#tool-output')).toContainText('expected a number', { timeout: 15000 });
});
