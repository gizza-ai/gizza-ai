import { test, expect } from './fixtures';

test('csv-join page inner join', async ({ page }) => {
  await page.goto('/tools/csv-join/');
  await page.fill('#in-left', 'id,name\n1,Alice\n2,Bob\n3,Carol');
  await page.fill('#in-right', 'id,city\n2,Berlin\n3,Cairo\n4,Delhi');
  await page.fill('#in-left_key', 'id');
  // Assert the exact multi-line joined CSV (textContent, not toHaveText which
  // collapses newlines to spaces).
  await expect(async () => {
    const out = await page.locator('#tool-output').textContent();
    expect(out).toBe('id,name,city\n2,Bob,Berlin\n3,Carol,Cairo\n');
  }).toPass({ timeout: 15000 });
});

test('csv-join query-param deep-link outer join', async ({ page }) => {
  const left = 'id,name\n1,Alice\n2,Bob\n3,Carol';
  const right = 'id,city\n2,Berlin\n3,Cairo\n4,Delhi';
  await page.goto(
    '/tools/csv-join/?left=' + encodeURIComponent(left) +
    '&right=' + encodeURIComponent(right) +
    '&left_key=id&join_type=outer'
  );
  await expect(page.locator('#in-left_key')).toHaveValue('id', { timeout: 15000 });
  await expect(async () => {
    const out = await page.locator('#tool-output').textContent();
    expect(out).toBe('id,name,city\n1,Alice,\n2,Bob,Berlin\n3,Carol,Cairo\n4,,Delhi\n');
  }).toPass({ timeout: 15000 });
});
