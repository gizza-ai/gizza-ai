import { test, expect } from './fixtures';

test('csv-anti-join page returns A-only unmatched rows by key', async ({ page }) => {
  await page.goto('/tools/csv-anti-join/');
  await page.fill('#in-a', 'id,name\n1,Alice\n2,Bob\n3,Carol');
  await page.fill('#in-b', 'id,city\n2,Berlin\n3,Cairo\n4,Delhi');
  await page.fill('#in-key', 'id');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Alice', { timeout: 15000 });
  expect(await out.textContent()).toBe('id,name\n1,Alice\n');
});

test('csv-anti-join page supports B-only via deep link', async ({ page }) => {
  const qs =
    '?a=' + encodeURIComponent('id,name\n1,Alice\n2,Bob\n3,Carol') +
    '&b=' + encodeURIComponent('id,city\n2,Berlin\n3,Cairo\n4,Delhi') +
    '&key=id' +
    '&direction=b-only' +
    '&delimiter=' + encodeURIComponent(',') +
    '&case_sensitive=true' +
    '&trim_keys=false';
  await page.goto('/tools/csv-anti-join/' + qs);

  await expect(page.locator('#in-direction')).toHaveValue('b-only', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Delhi', { timeout: 15000 });
  expect(await out.textContent()).toBe('id,city\n4,Delhi\n');
});

test('csv-anti-join page emits both sides for composite keys and preserves duplicate unmatched rows', async ({ page }) => {
  await page.goto('/tools/csv-anti-join/');
  await page.fill('#in-a', 'first,last,plan\nAda,Lovelace,pro\nAlan,Turing,basic\nAlan,Turing,plus');
  await page.fill('#in-b', 'first,last,plan\nAda,Lovelace,pro\nGrace,Hopper,team');
  await page.fill('#in-key', 'first,last');
  await page.selectOption('#in-direction', 'both');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Alan', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    '_source,first,last,plan\n' +
      'A,Alan,Turing,basic\n' +
      'A,Alan,Turing,plus\n' +
      'B,Grace,Hopper,team\n',
  );
});

test('csv-anti-join page supports non-default case-insensitive trimmed key matching', async ({ page }) => {
  await page.goto('/tools/csv-anti-join/');
  await page.fill('#in-a', 'id,name\n A1 ,Alice\nB2,Bob');
  await page.fill('#in-b', 'id\na1');
  await page.fill('#in-key', 'id');
  await page.uncheck('#in-case_sensitive');
  await page.check('#in-trim_keys');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Bob', { timeout: 15000 });
  expect(await out.textContent()).toBe('id,name\nB2,Bob\n');
});

test('csv-anti-join page reports a missing key column error', async ({ page }) => {
  await page.goto('/tools/csv-anti-join/');
  await page.fill('#in-a', 'id,name\n1,Alice');
  await page.fill('#in-b', 'id\n1');
  await page.fill('#in-key', 'missing');

  await expect(page.locator('#tool-output')).toContainText("A key column 'missing' not found", { timeout: 15000 });
});
