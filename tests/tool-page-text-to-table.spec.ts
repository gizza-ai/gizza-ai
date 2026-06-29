import { test, expect } from './fixtures';

async function fillText(page: any, selector: string, value: string) {
  await page.$eval(
    selector,
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('text-to-table page renders an aligned ASCII table', async ({ page }) => {
  await page.goto('/tools/text-to-table/');
  await fillText(page, '#in-data', 'name,score\nAlice,10\nBob,9');
  await page.fill('#in-delimiter', ',');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('+-------+-------+', { timeout: 15000 });
  await expect(out).toContainText('| name  | score |');
  await expect(out).toContainText('| Alice | 10    |');
});

test('text-to-table page renders Markdown from TSV', async ({ page }) => {
  await page.goto('/tools/text-to-table/');
  await page.selectOption('#in-format', 'markdown');
  await page.selectOption('#in-align', 'right');
  await page.fill('#in-delimiter', 'tab');
  await fillText(page, '#in-data', 'name\tscore\nAlice\t10\nBob\t9');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('|  name | score |', { timeout: 15000 });
  await expect(out).toContainText('| ----: | ----: |');
  await expect(out).toContainText('| Alice |    10 |');
});
