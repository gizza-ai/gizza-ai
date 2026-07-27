import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const CSV_DATA = 'email,age,plan\na@b.com,34,pro\nbad-email,200,gold';
const RULES = 'email:required\nemail:type=email\nage:type=int\nage:min=0\nage:max=120\nplan:enum=free|pro|team';
const CSV_REPORT =
  'INVALID — 3 violation(s) across 2 record(s).\n' +
  'CSV · delimiter comma · 2 record(s) · 6 rule(s) · 12 check(s)\n' +
  'Fields checked: email, age, plan\n' +
  'Record 2 (line 3), field "email" [type=email] — "bad-email" is not a valid email\n' +
  'Record 2 (line 3), field "age" [max=120] — 200 is greater than max 120\n' +
  'Record 2 (line 3), field "plan" [enum(free|pro|team)] — "gold" is not one of free, pro, team';

test('data-validator page — validates CSV and exact report', async ({ page }) => {
  await page.goto('/tools/data-validator/');
  await page.fill('#in-data', CSV_DATA);
  await page.fill('#in-rules', RULES);
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-delimiter', 'comma');
  await page.fill('#in-max_issues', '50');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toContainText('INVALID — 3 violation', { timeout: 15000 });
  expect(await outputText(page)).toBe(CSV_REPORT);
});

test('data-validator page — JSON rows and unique rule', async ({ page }) => {
  await page.goto('/tools/data-validator/');
  await page.fill('#in-data', '[{"id":1,"email":"a@b.com"},{"id":1,"email":"b@c.com"}]');
  await page.fill('#in-rules', 'id:unique\nid:type=int\nemail:type=email');
  await page.selectOption('#in-input_format', 'json');
  await page.fill('#in-max_issues', '50');
  await expect(page.locator('#tool-output')).toContainText('INVALID — 1 violation', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('field "id" [unique]');
});

test('data-validator page — semicolon CSV and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/data-validator/');
  await page.fill('#in-data', 'AB12;Widget\n;Gadget\n;');
  await page.fill('#in-rules', '1:required\n1:minlen=4\n2:required');
  await page.selectOption('#in-input_format', 'csv');
  await page.uncheck('#in-header');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.fill('#in-max_issues', '1');
  await expect(page.locator('#tool-output')).toContainText('INVALID — 3 violation', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('(+ 2 more violation(s) not shown');
});

test('data-validator page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/data-validator/?data=' +
      encodeURIComponent(CSV_DATA) +
      '&rules=' +
      encodeURIComponent(RULES) +
      '&input_format=csv&header=true&delimiter=comma&max_issues=50&format=text',
  );
  await expect(page.locator('#in-data')).toHaveValue(CSV_DATA, { timeout: 15000 });
  await expect(page.locator('#in-rules')).toHaveValue(RULES);
  await expect(page.locator('#tool-output')).toContainText('INVALID — 3 violation', { timeout: 15000 });
  expect(await outputText(page)).toBe(CSV_REPORT);
});
