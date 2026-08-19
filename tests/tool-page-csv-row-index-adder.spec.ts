import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('csv-row-index-adder adds default sequential index column', async ({ page }) => {
  await page.goto('/tools/csv-row-index-adder/');
  await setBigTextarea(page, '#in-data', 'name,city\nAda,London\nLin,Taipei');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,name,city', { timeout: 15000 });
  await expect(out).toContainText('1,Ada,London');
  await expect(out).toContainText('2,Lin,Taipei');
});

test('csv-row-index-adder deep link builds padded invoice numbers', async ({ page }) => {
  await page.goto('/tools/csv-row-index-adder/?mode=sequential&column_name=invoice&start=7&step=5&pad_width=4&prefix=INV-&position=end');
  await expect(page.locator('#in-column_name')).toHaveValue('invoice', { timeout: 15000 });
  await expect(page.locator('#in-position')).toHaveValue('end');
  await setBigTextarea(page, '#in-data', 'customer,total\nAda,42\nLin,99');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('customer,total,invoice', { timeout: 15000 });
  await expect(out).toContainText('Ada,42,INV-0007');
  await expect(out).toContainText('Lin,99,INV-0012');
});

test('csv-row-index-adder supports composite keys and header checkbox off', async ({ page }) => {
  await page.goto('/tools/csv-row-index-adder/');
  await setBigTextarea(page, '#in-data', 'EU,ops,10\nUS,eng,20');
  await page.selectOption('#in-mode', 'composite');
  await page.uncheck('#in-has_header');
  await page.fill('#in-column_name', 'key');
  await page.fill('#in-columns', '1,2');
  await page.fill('#in-separator', '::');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('EU::ops,EU,ops,10', { timeout: 15000 });
  await expect(out).toContainText('US::eng,US,eng,20');
});
