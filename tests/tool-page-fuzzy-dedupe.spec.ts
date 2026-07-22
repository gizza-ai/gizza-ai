import { test, expect } from './fixtures';

const CITY_LIST = 'New York\nnew york\nNew  York\nNwe York\nBoston\nBoston\nboston';

test('fuzzy-dedupe page removes typo/case/spacing near-duplicates', async ({ page }) => {
  await page.goto('/tools/fuzzy-dedupe/');
  await page.fill('#in-data', CITY_LIST);
  await page.fill('#in-threshold', '70');
  await expect(page.locator('#tool-output')).toContainText('New York', { timeout: 15000 });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  expect(out).toContain('New York');
  expect(out).toContain('Boston');
  // Near-duplicate variants are dropped, not kept.
  expect(out).not.toContain('Nwe York');
  expect(out).not.toContain('new york');
});

test('fuzzy-dedupe deep-link keys on a CSV column and keeps the longest row', async ({ page }) => {
  const data = 'company,amount\nAcme Inc,100\nacme inc,90\nAcme  Inc,80\nGlobex,50';
  await page.goto(
    '/tools/fuzzy-dedupe/?' +
      new URLSearchParams({
        data,
        columns: 'company',
        header: 'true',
        keep: 'longest',
        output: 'deduped',
      }).toString()
  );
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('company,amount', { timeout: 15000 });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  // keep=longest survivor of the Acme group is the longest full row ("Acme Inc,100").
  expect(out).toContain('Acme Inc,100');
  expect(out).toContain('Globex,50');
  expect(out).not.toContain('Acme  Inc,80');
});

test('fuzzy-dedupe output=json enum returns groups and stats', async ({ page }) => {
  await page.goto('/tools/fuzzy-dedupe/');
  await page.fill('#in-data', 'Apple\napple\napple\nBanana');
  await page.selectOption('#in-output', 'json');
  await expect(page.locator('#tool-output')).toContainText('"near_duplicate_groups": 1', {
    timeout: 15000,
  });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  expect(out).toContain('"total_rows": 4');
  expect(out).toContain('"removed_rows": 2');
});

test('fuzzy-dedupe output=removed lists only the dropped rows', async ({ page }) => {
  await page.goto('/tools/fuzzy-dedupe/');
  await page.fill('#in-data', 'New York\nnew york\nNwe York\nBoston');
  await page.fill('#in-threshold', '70');
  await page.selectOption('#in-output', 'removed');
  await expect(page.locator('#tool-output')).toContainText('new york', { timeout: 15000 });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  expect(out).toContain('Nwe York');
  expect(out).not.toContain('Boston');
});

// Non-default checkbox state: disabling case normalization keeps USA and usa apart at threshold 100.
test('fuzzy-dedupe normalize_case checkbox can be disabled', async ({ page }) => {
  await page.goto('/tools/fuzzy-dedupe/');
  await page.fill('#in-data', 'USA\nusa\nUSA');
  await page.fill('#in-threshold', '100');
  await page.uncheck('#in-normalize_case');
  await expect(page.locator('#tool-output')).toContainText('USA', { timeout: 15000 });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  // Case-sensitive at 100: USA collapses to one, usa stays separate.
  expect(out).toContain('usa');
});
