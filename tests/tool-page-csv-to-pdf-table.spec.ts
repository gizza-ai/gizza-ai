import { test, expect } from './fixtures';

const DATA = 'name,age,team\nAlice,30,East\nBob,25,West\nCarol,28,East';

async function pdfInfo(page) {
  const text = ((await page.locator('#tool-output').textContent()) ?? '').trim();
  expect(text).toMatch(/^data:application\/pdf;base64,/);
  return page.evaluate((url) => {
    const b64 = url.replace(/^data:application\/pdf;base64,/, '');
    const bin = atob(b64);
    return {
      header: bin.slice(0, 5),
      bytes: bin.length,
      mediaBoxes: (bin.match(/\/MediaBox/g) ?? []).length,
      hasCatalog: bin.includes('/Type /Catalog'),
    };
  }, text);
}

test('csv-to-pdf-table generates a real PDF data URL', async ({ page }) => {
  await page.goto('/tools/csv-to-pdf-table/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-title', 'Team roster');
  await page.selectOption('#in-page_size', 'letter');
  await page.selectOption('#in-orientation', 'portrait');
  await page.fill('#in-font_size', '10');

  const info = await pdfInfo(page);
  expect(info.header).toBe('%PDF-');
  expect(info.bytes).toBeGreaterThan(700);
  expect(info.mediaBoxes).toBe(1);
  expect(info.hasCatalog).toBe(true);
});

test('csv-to-pdf-table supports tab delimiter and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/csv-to-pdf-table/');
  await page.fill('#in-data', 'Alice\t30\tEast\nBob\t25\tWest');
  await page.selectOption('#in-delimiter', 'tab');
  await page.uncheck('#in-header');
  await page.uncheck('#in-grid');
  await page.selectOption('#in-orientation', 'landscape');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-grid')).not.toBeChecked();

  const info = await pdfInfo(page);
  expect(info.header).toBe('%PDF-');
  expect(info.bytes).toBeGreaterThan(500);
});

test('csv-to-pdf-table deep-links params and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    delimiter: 'comma',
    header: 'true',
    title: 'Deep link roster',
    page_size: 'a4',
    orientation: 'landscape',
    font_size: '9',
    row_banding: 'true',
    grid: 'true',
  });
  await page.goto(`/tools/csv-to-pdf-table/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(DATA, { timeout: 15000 });
  await expect(page.locator('#in-page_size')).toHaveValue('a4');
  await expect(page.locator('#in-orientation')).toHaveValue('landscape');
  const info = await pdfInfo(page);
  expect(info.header).toBe('%PDF-');
});
