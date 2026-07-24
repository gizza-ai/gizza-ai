import { test, expect } from './fixtures';

const meanCsv = 'name,age,city\nAlice,30,NYC\nBob,,LA\nCarol,40,';

test('missing-value-imputer page mean-fills a numeric CSV column', async ({ page }) => {
  await page.goto('/tools/missing-value-imputer/');
  await page.fill('#in-input', meanCsv);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Bob,35,LA', { timeout: 15_000 });
  expect(await out.textContent()).toBe('name,age,city\nAlice,30,NYC\nBob,35,LA\nCarol,40,\n');
});

test('missing-value-imputer deep link covers non-default enum and checkbox controls', async ({ page }) => {
  const csv = '1|\n|2';
  const qs =
    '?input=' + encodeURIComponent(csv) +
    '&strategy=constant' +
    '&header=false' +
    '&delimiter=pipe' +
    '&fill_value=0';
  await page.goto('/tools/missing-value-imputer/' + qs);

  await expect(page.locator('#in-strategy')).toHaveValue('constant', { timeout: 15_000 });
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-delimiter')).toHaveValue('pipe');
  await expect(page.locator('#in-fill_value')).toHaveValue('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1|0', { timeout: 15_000 });
  expect(await out.textContent()).toBe('1|0\n0|2\n');
});

test('missing-value-imputer page handles most-frequent categorical imputation', async ({ page }) => {
  await page.goto('/tools/missing-value-imputer/');
  await page.fill('#in-input', 'color,n\nred,1\nred,2\n,3\nblue,4');
  await page.selectOption('#in-strategy', 'most_frequent');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('red,3', { timeout: 15_000 });
  expect(await out.textContent()).toBe('color,n\nred,1\nred,2\nred,3\nblue,4\n');
});
