import { test, expect } from './fixtures';

const CITY_LIST = 'New York\nnew york\nNew  York\nNwe York\nBoston\nBoston\nboston';

test('cluster-similar-values page clusters typo/case/spacing variants', async ({ page }) => {
  await page.goto('/tools/cluster-similar-values/');
  await page.fill('#in-data', CITY_LIST);
  await page.fill('#in-threshold', '70');
  await expect(page.locator('#tool-output')).toContainText('7 values, 6 unique → 2 clusters', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('### New York');
  expect(out).toContain('| new york | New York | 1 |');
  expect(out).toContain('| Nwe York | New York | 1 |');
  expect(out).toContain('| boston | Boston | 1 |');
});

test('cluster-similar-values deep-link prefills CSV column and emits mapping CSV', async ({ page }) => {
  const data = 'company,amount\nAcme Inc,100\nacme inc,90\nAcme  Inc,80\nGlobex,50';
  await page.goto(
    '/tools/cluster-similar-values/?' +
      new URLSearchParams({
        data,
        column: 'company',
        header: 'true',
        output: 'csv',
      }).toString()
  );
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('cluster,original,canonical,count', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('1,Acme Inc,Acme Inc,1');
  expect(out).toContain('1,acme inc,Acme Inc,1');
  expect(out).toContain('1,Acme  Inc,Acme Inc,1');
  expect(out).toContain('2,Globex,Globex,1');
});

test('cluster-similar-values output=json enum returns structured clusters', async ({ page }) => {
  await page.goto('/tools/cluster-similar-values/');
  await page.fill('#in-data', 'Apple\napple\napple\nBanana');
  await page.selectOption('#in-output', 'json');
  await expect(page.locator('#tool-output')).toContainText('"canonical": "apple"', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('"near_duplicate_clusters": 1');
});

// Non-default checkbox state: disabling case normalization keeps USA and usa apart at threshold 100.
test('cluster-similar-values normalize_case checkbox can be disabled', async ({ page }) => {
  await page.goto('/tools/cluster-similar-values/');
  await page.fill('#in-data', 'USA\nusa\nUSA');
  await page.fill('#in-threshold', '100');
  await page.uncheck('#in-normalize_case');
  await expect(page.locator('#tool-output')).toContainText('3 values, 2 unique → 2 clusters', {
    timeout: 15000,
  });
});

// Cap/boundary: threshold=100 is accepted and only identical normalized values merge.
test('cluster-similar-values threshold boundary 100 is accepted', async ({ page }) => {
  await page.goto('/tools/cluster-similar-values/');
  await page.fill('#in-data', 'abc\nabd\nabc');
  await page.fill('#in-threshold', '100');
  await expect(page.locator('#tool-output')).toContainText('3 values, 2 unique → 2 clusters', {
    timeout: 15000,
  });
});
