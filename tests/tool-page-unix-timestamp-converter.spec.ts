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

test('unix-timestamp-converter page converts seconds to UTC date', async ({ page }) => {
  await page.goto('/tools/unix-timestamp-converter/');
  await fillText(page, '#in-value', '1700000000');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"direction": "timestamp-to-date"', { timeout: 15000 });
  await expect(out).toContainText('"detected_unit": "seconds"');
  await expect(out).toContainText('"utc": "2023-11-14 22:13:20 UTC"');
});

test('unix-timestamp-converter page auto-detects milliseconds', async ({ page }) => {
  await page.goto('/tools/unix-timestamp-converter/');
  await fillText(page, '#in-value', '1700000000000');
  await expect(page.locator('#tool-output')).toContainText('"detected_unit": "milliseconds"', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('"seconds": 1700000000');
});

test('unix-timestamp-converter page converts date with offset to timestamp', async ({ page }) => {
  await page.goto('/tools/unix-timestamp-converter/');
  await page.selectOption('#in-mode', 'to-timestamp');
  await fillText(page, '#in-value', '2023-11-15T00:13:20+02:00');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"direction": "date-to-timestamp"', { timeout: 15000 });
  await expect(out).toContainText('"assumed_utc": false');
  await expect(out).toContainText('"seconds": 1700000000');
});

test('unix-timestamp-converter query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/unix-timestamp-converter/?value=' +
      encodeURIComponent('1970-01-01') +
      '&mode=to-timestamp&unit=auto',
  );
  await expect(page.locator('#in-value')).toHaveValue('1970-01-01', { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('to-timestamp');
  await expect(page.locator('#tool-output')).toContainText('"seconds": 0', { timeout: 15000 });
});
