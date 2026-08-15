import { test, expect } from './fixtures';

const tool = '/tools/rule-based-extractor/';
const invoiceText = 'Invoice INV-2026-014 dated 2026-08-15, billed to ada@example.com, total $1,240.00 (VAT 21%).';
const invoiceRules = 'invoice = INV-\\d{4}-\\d+\ndate = %{DATE_ISO}\nemail = %{EMAIL}\ntotal = %{MONEY}\nvat = %{PERCENT}';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('rule-based-extractor page extracts invoice fields as exact JSON', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', invoiceText);
  await page.fill('#in-rules', invoiceRules);
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(
    '{"invoice":"INV-2026-014","date":"2026-08-15","email":"ada@example.com","total":"$1,240.00","vat":"21%"}',
    { timeout: 15000 },
  );
});

test('rule-based-extractor page splits lines into JSON records', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'id=1 ok\nnothing here\nid=2 ok');
  await page.fill('#in-rules', 'id = id=(\\d+)');
  await page.selectOption('#in-split', 'lines');

  await expect(page.locator('#tool-output')).toHaveText('[{"id":"1"},{"id":"2"}]', {
    timeout: 15000,
  });
});

test('rule-based-extractor page supports all matches, unique, and pretty JSON', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'a@x.com, b@y.org, a@x.com');
  await page.fill('#in-rules', 'email = %{EMAIL}');
  await page.selectOption('#in-matches', 'all');
  await page.check('#in-unique');
  await page.check('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "email": [\n    "a@x.com",\n    "b@y.org"\n  ]\n}',
    { timeout: 15000 },
  );
});

test('rule-based-extractor query-param deep-link prefills and computes report', async ({ page }) => {
  const text = '2026-08-15 hello';
  const rules = 'date = %{DATE_ISO}\nuser = user=(\\w+)';
  await page.goto(
    tool +
      '?text=' +
      encodeURIComponent(text) +
      '&rules=' +
      encodeURIComponent(rules) +
      '&split=lines&output=report',
  );

  await expect(page.locator('#in-text')).toHaveValue(text, { timeout: 15000 });
  await expect(page.locator('#in-rules')).toHaveValue(rules);
  await expect(page.locator('#in-split')).toHaveValue('lines');
  await expect(page.locator('#in-output')).toHaveValue('report');
  expect(await outputText(page)).toContain('Never matched: user');
  expect(await outputText(page)).toContain('Values extracted: 1');
});
