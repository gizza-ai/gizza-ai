import { test, expect } from './fixtures';

const LEFT = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJyb2xlIjoidXNlciIsInNjb3BlIjpbInJlYWQiXSwiZXhwIjoxNTE2MjM5MDIyfQ.sig';
const RIGHT = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJyb2xlIjoiYWRtaW4iLCJzY29wZSI6WyJyZWFkIiwid3JpdGUiXSwicGxhbiI6InBybyIsImV4cCI6MTUxNjI0MjYyMn0.sig';
const RIGHT_HEADER = 'eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIxMjMiLCJyb2xlIjoiYWRtaW4iLCJzY29wZSI6WyJyZWFkIiwid3JpdGUiXSwicGxhbiI6InBybyIsImV4cCI6MTUxNjI0MjYyMn0.sig';

async function outputJson(page) {
  const text = (await page.locator('#tool-output').textContent())?.trim() ?? '';
  return JSON.parse(text);
}

test('jwt-claims-diff reports added and changed payload claims exactly', async ({ page }) => {
  await page.goto('/tools/jwt-claims-diff/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT);
  await page.fill('#in-indent', '2');
  await expect(page.locator('#tool-output')).toContainText('"equal": false', { timeout: 15000 });
  const report = await outputJson(page);
  expect(report.equal).toBe(false);
  expect(report.summary.added).toBe(1);
  expect(report.summary.removed).toBe(0);
  expect(report.summary.changed).toBe(3);
  expect(report.payload).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ claim: 'role', kind: 'changed', old: 'user', new: 'admin' }),
      expect.objectContaining({ claim: 'scope', kind: 'changed', old: ['read'], new: ['read', 'write'] }),
      expect.objectContaining({ claim: 'plan', kind: 'added', new: 'pro' }),
    ]),
  );
  expect(report.expiry.delta_seconds).toBe(3600);
});

test('jwt-claims-diff can ignore header changes and minify output', async ({ page }) => {
  await page.goto('/tools/jwt-claims-diff/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT_HEADER);
  await page.uncheck('#in-include_header');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toContainText('{"equal":false', { timeout: 15000 });
  const report = await outputJson(page);
  expect(report.header).toBeUndefined();
  expect(report.summary.changed).toBe(3);
});

test('jwt-claims-diff deep-link pre-fills and runs', async ({ page }) => {
  const l = encodeURIComponent(LEFT);
  const r = encodeURIComponent(RIGHT);
  await page.goto(`/tools/jwt-claims-diff/?left=${l}&right=${r}&indent=2&include_header=true`);
  await expect(page.locator('#tool-output')).toContainText('"claim": "role"', { timeout: 15000 });
  const report = await outputJson(page);
  expect(report.payload.some((c) => c.claim === 'role' && c.new === 'admin')).toBe(true);
});

test('jwt-claims-diff reports invalid JWT input', async ({ page }) => {
  await page.goto('/tools/jwt-claims-diff/');
  await page.fill('#in-left', 'not-a-jwt');
  await page.fill('#in-right', RIGHT);
  await expect(page.locator('#tool-output')).toContainText('could not be decoded', { timeout: 15000 });
});
