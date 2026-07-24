import { test, expect } from './fixtures';

const BAD_CSV = `name,age,score,active,joined,plan
Ada,34,9.5,true,2021-06-01,pro
Bo,twelve,8,yes,2021/06/02,gold`;
const TYPES = 'age:int, score:float, active:bool, joined:date, plan:enum(free|pro)';

test('csv-column-type-validator page lists offending row and columns', async ({ page }) => {
  await page.goto('/tools/csv-column-type-validator/');
  await page.fill('#in-data', BAD_CSV);
  await page.fill('#in-types', TYPES);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — 3 offending cell(s).', { timeout: 15000 });
  await expect(out).toContainText('Row 2 (line 3), column "age"');
  await expect(out).toContainText('Row 2 (line 3), column "joined"');
  await expect(out).toContainText('Row 2 (line 3), column "plan"');
});

test('csv-column-type-validator deep-link validates headerless pipe data', async ({ page }) => {
  const data = 'Ada|34|pro\nBo|x|gold';
  await page.goto(
    '/tools/csv-column-type-validator/?' +
      new URLSearchParams({
        data,
        types: '2:int, 3:enum(free|pro)',
        header: 'false',
        delimiter: 'pipe',
        date_format: 'iso',
        empty_ok: 'true',
        max_issues: '50',
        format: 'text',
      }).toString()
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — 2 offending cell(s).', { timeout: 15000 });
  await expect(out).toContainText('column "col 2"');
  await expect(out).toContainText('column "col 3"');
});

test('csv-column-type-validator JSON output includes structured issues', async ({ page }) => {
  await page.goto('/tools/csv-column-type-validator/');
  await page.fill('#in-data', 'id,paid\n1,true\n2,maybe');
  await page.fill('#in-types', 'id:int, paid:bool');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15000 });
  await expect(out).toContainText('"offending_count": 1');
  await expect(out).toContainText('"column": "paid"');
});

test('csv-column-type-validator empty_ok checkbox can require blanks', async ({ page }) => {
  await page.goto('/tools/csv-column-type-validator/');
  await page.fill('#in-data', 'id,age\n1,\n2,30');
  await page.fill('#in-types', 'age:int');
  await page.uncheck('#in-empty_ok');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — 1 offending cell(s).', { timeout: 15000 });
  await expect(out).toContainText('empty cell (required)');
});

test('csv-column-type-validator date_format enum accepts US dates', async ({ page }) => {
  await page.goto('/tools/csv-column-type-validator/');
  await page.fill('#in-data', 'joined\n06/15/2021\n15/06/2021');
  await page.fill('#in-types', 'joined:date');
  await page.selectOption('#in-date_format', 'us');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — 1 offending cell(s).', { timeout: 15000 });
  await expect(out).toContainText('Row 2 (line 3), column "joined"');
});
