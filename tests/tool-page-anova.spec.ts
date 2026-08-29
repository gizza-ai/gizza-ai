import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setData(page: any, value: string) {
  await page.locator('#in-data').evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const WIDE = '5,5,8\n6,7,11\n9,9,13\n9,10,13\n11,11,14';

test('anova page renders the exact textbook ANOVA headline', async ({ page }) => {
  await page.goto('/tools/anova/');
  await setData(page, WIDE);
  await page.selectOption('#in-format', 'wide');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-header', 'no');
  await page.fill('#in-alpha', '0.05');
  await page.fill('#in-decimals', '4');
  await page.selectOption('#in-posthoc', 'none');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toContainText('F(2, 12) = 3.7371, p = 0.0547', {
    timeout: 15000,
  });
  const text = await outText(page);
  expect(text).toContain('groups: 3');
  expect(text).toContain('observations: 15');
  expect(text).toContain('critical F at alpha 0.0500 = 3.8853');
  expect(text).toContain('fail to reject the null hypothesis');
  expect(text).toContain('eta-squared = 0.3838 (large)');
});

test('anova page deep-link pre-fills controls and renders Tukey pairs', async ({ page }) => {
  const long =
    'group,value\nControl,5\nControl,6\nControl,9\nControl,9\nControl,11\nDrug A,5\nDrug A,7\nDrug A,9\nDrug A,10\nDrug A,11\nDrug B,8\nDrug B,11\nDrug B,13\nDrug B,13\nDrug B,14';
  await page.goto(
    '/tools/anova/?data=' +
      encodeURIComponent(long) +
      '&format=long&delimiter=comma&header=yes&alpha=0.05&decimals=4&posthoc=tukey&output=summary',
  );

  await expect(page.locator('#in-data')).toHaveValue(long, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('long');
  await expect(page.locator('#in-posthoc')).toHaveValue('tukey');
  await expect(page.locator('#tool-output')).toContainText('Post-hoc: Tukey HSD', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Control vs Drug B');
  expect(text).toContain('p adj');
});

test('anova page advertised output modes include summary statistics json', async ({ page }) => {
  await page.goto('/tools/anova/');
  await setData(page, 'name,n,mean,sd\nControl,5,8.0,2.4495\nDrug A,5,8.4,2.4083\nDrug B,5,11.8,2.3875');
  await page.selectOption('#in-format', 'summary');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-header', 'yes');
  await page.selectOption('#in-posthoc', 'holm');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toContainText('"test": "one-way ANOVA"', {
    timeout: 15000,
  });
  const text = await outText(page);
  expect(text).toContain('"input_format": "summary"');
  expect(text).toContain('"posthoc": "holm"');
  expect(text).toContain('summary statistics, so Levene');
});

test('anova page reports helpful errors for invalid data and cap-adjacent input', async ({ page }) => {
  await page.goto('/tools/anova/');
  await setData(page, '1,2\n3,oops');
  await page.selectOption('#in-format', 'wide');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-header', 'no');
  await expect(page.locator('#tool-output')).toContainText('line 2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('oops', { timeout: 15000 });

  await setData(page, '1,2\n2,3\n3,4');
  await expect(page.locator('#tool-output')).toContainText('F(1, 4)', { timeout: 15000 });
});
