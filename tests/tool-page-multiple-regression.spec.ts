import { test, expect } from './fixtures';

const houseData = '1,1,6\n2,1,8\n3,2,11\n4,2,14\n5,3,18\n6,3,20';

test('multiple-regression fits two predictors and reports model statistics', async ({ page }) => {
  await page.goto('/tools/multiple-regression/');
  await page.fill('#in-data', houseData);
  await page.fill('#in-labels', 'sqft,rooms,price');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('price = 2 + 2.333333·sqft + 1.333333·rooms', { timeout: 15000 });
  await expect(out).toContainText('R-squared: 0.995638');
  await expect(out).toContainText('Adjusted R-squared: 0.99273');
  await expect(out).toContainText('F-statistic: 342.375 on 2 and 3 DF');
  await expect(out).toContainText('sqft');
  await expect(out).toContainText('rooms');
});

test('format=json includes fitted values and residuals', async ({ page }) => {
  await page.goto('/tools/multiple-regression/');
  await page.fill('#in-data', '1 2\n2 4\n3 5\n4 4\n5 5');
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"r_squared": 0.6', { timeout: 15000 });
  await expect(out).toContainText('"fitted"');
  await expect(out).toContainText('"residuals"');
  await expect(out).toContainText('"estimate": 2.2');
});

test('response=first uses the first column as Y', async ({ page }) => {
  await page.goto('/tools/multiple-regression/');
  await page.fill('#in-data', '6 1 1\n8 1 2\n11 2 3\n14 2 4');
  await page.fill('#in-response', 'first');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('v1 = ', { timeout: 15000 });
  await expect(out).toContainText('Predictors: 2');
});

test('unchecking intercept forces a no-intercept fit', async ({ page }) => {
  await page.goto('/tools/multiple-regression/');
  await page.fill('#in-data', '1 2\n2 4.1\n3 5.9\n4 8.2');
  await page.uncheck('#in-intercept');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('v2 = 2.023333·v1', { timeout: 15000 });
  await expect(out).not.toContainText('(Intercept)');
});

test('confidence level boundary changes the displayed CI header', async ({ page }) => {
  await page.goto('/tools/multiple-regression/');
  await page.fill('#in-data', '1 2\n2 4\n3 5\n4 4\n5 5');
  await page.fill('#in-conf_level', '0.5');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('50% CI', { timeout: 15000 });
});

test('query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto(
    '/tools/multiple-regression/?data=' +
      encodeURIComponent(houseData) +
      '&labels=' +
      encodeURIComponent('sqft,rooms,price') +
      '&format=json',
  );
  await expect(page.locator('#in-data')).toHaveValue(houseData, { timeout: 15000 });
  await expect(page.locator('#in-labels')).toHaveValue('sqft,rooms,price');
  await expect(page.locator('#in-format')).toHaveValue('json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"response": "price"', { timeout: 15000 });
  await expect(out).toContainText('"r_squared": 0.995638');
});
