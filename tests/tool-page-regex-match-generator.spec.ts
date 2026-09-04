import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('regex-match-generator creates deterministic seeded samples', async ({ page }) => {
  await page.goto('/tools/regex-match-generator/');
  await setField(page, '#in-pattern', '[A-Z]{3}-\\d{4}');

  await expect(page.locator('#tool-output')).toHaveText(
    'ILJ-7801\nLGM-6373\nEUZ-0696\nZLQ-0670\nWKR-4519\n',
    { timeout: 15_000 },
  );
});

test('regex-match-generator honors deep-linked sequential CSV parameters', async ({ page }) => {
  const params = new URLSearchParams({
    pattern: '(red|green|blue)-[0-2]',
    count: '4',
    style: 'sequential',
    seed: '42',
    max_repeat: '4',
    max_length: '200',
    unique: 'true',
    output: 'csv',
  });
  await page.goto(`/tools/regex-match-generator/?${params.toString()}`);

  await expect(page.locator('#in-style')).toHaveValue('sequential');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toHaveText(
    'index,sample\n1,"red-0"\n2,"green-0"\n3,"blue-0"\n4,"red-1"\n',
    { timeout: 15_000 },
  );
});

test('regex-match-generator covers style/output, checkbox and boundary controls', async ({ page }) => {
  await page.goto('/tools/regex-match-generator/');
  await setField(page, '#in-pattern', 'ab+');
  await setField(page, '#in-count', '1');
  await page.selectOption('#in-style', 'longest');
  await setField(page, '#in-max_repeat', '3');
  await setField(page, '#in-max_length', '10');
  await page.uncheck('#in-unique');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"style": "longest"', { timeout: 15_000 });
  await expect(out).toContainText('"unique": false');
  await expect(out).toContainText('"samples": [\n    "abbb"\n  ]');

  await setField(page, '#in-pattern', '(yes|no)');
  await page.selectOption('#in-style', 'shortest');
  await page.selectOption('#in-output', 'lines');
  await page.check('#in-unique');
  await expect(page.locator('#tool-output')).toHaveText('yes\n');

  await page.selectOption('#in-style', 'random');
  await setField(page, '#in-count', '200');
  await setField(page, '#in-pattern', '[0-9]');
  await page.uncheck('#in-unique');
  await expect(page.locator('#tool-output')).toContainText('0', { timeout: 15_000 });
});
