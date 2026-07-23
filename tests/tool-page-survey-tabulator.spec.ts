import { test, expect } from './fixtures';

const csv = 'gender,satisfaction,region\nF,High,North\nM,Low,South\nF,High,North\nF,Medium,North\nM,High,South\nM,Low,South\nF,,North';

async function outputText(page: any) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('satisfaction', { timeout: 15000 });
  const text = await out.textContent();
  expect(text).toBeTruthy();
  return text!;
}

test('survey-tabulator page builds an overview frequency table', async ({ page }) => {
  await page.goto('/tools/survey-tabulator/');
  await page.fill('#in-data', csv);
  await page.selectOption('#in-mode', 'overview');
  await page.fill('#in-question', 'satisfaction');
  await page.fill('#in-by', '');
  await page.selectOption('#in-percent', 'total');
  await page.fill('#in-top', '0');
  await page.fill('#in-delimiter', ',');

  const text = await outputText(page);
  expect(text).toContain('satisfaction — 6 responses (1 blank, 3 distinct)');
  expect(text).toContain('High');
  expect(text).toContain('50.0%');
  expect(text).toContain('Total');
});

test('survey-tabulator exercises enum values and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/survey-tabulator/');
  await page.fill('#in-data', csv);
  await page.selectOption('#in-mode', 'crosstab');
  await page.fill('#in-question', 'gender');
  await page.fill('#in-by', 'satisfaction');
  await page.fill('#in-delimiter', ',');

  for (const percent of ['total', 'row', 'column', 'none']) {
    await page.selectOption('#in-percent', percent);
    const text = await outputText(page);
    expect(text, percent).toContain('Crosstab: gender × satisfaction');
    expect(text, percent).toContain('Total');
  }

  await page.check('#in-stats');
  await page.selectOption('#in-percent', 'row');
  let text = await outputText(page);
  expect(text).toContain('Chi-square =');
  expect(text).toContain("Cramér's V =");

  await page.uncheck('#in-stats');
  await page.check('#in-include_blanks');
  text = await outputText(page);
  expect(text).toContain('(blank)');

  await page.selectOption('#in-sort', 'label');
  text = await outputText(page);
  expect(text).toContain('Crosstab: gender × satisfaction');
});

test('survey-tabulator handles delimiter variants and top boundary', async ({ page }) => {
  await page.goto('/tools/survey-tabulator/');
  const pipe = 'gender|satisfaction|region\nF|High|North\nM|Low|South\nF|High|North';
  await page.fill('#in-data', pipe);
  await page.selectOption('#in-mode', 'overview');
  await page.fill('#in-question', 'satisfaction');
  await page.fill('#in-by', '');
  await page.fill('#in-delimiter', 'pipe');
  await page.fill('#in-top', '1');

  let text = await outputText(page);
  expect(text).toContain('showing top 1 of 2');
  expect(text).toContain('High');

  await page.fill('#in-top', '0');
  text = await outputText(page);
  expect(text).toContain('Low');
});

test('survey-tabulator deep-link prefills params and computes', async ({ page }) => {
  const qs =
    '?data=' + encodeURIComponent(csv) +
    '&mode=crosstab&question=gender&by=satisfaction&percent=row' +
    '&include_blanks=true&stats=true&sort=label&top=0&delimiter=%2C';
  await page.goto('/tools/survey-tabulator/' + qs);

  await expect(page.locator('#in-question')).toHaveValue('gender', { timeout: 15000 });
  await expect(page.locator('#in-by')).toHaveValue('satisfaction');
  await expect(page.locator('#in-percent')).toHaveValue('row');
  await expect(page.locator('#in-include_blanks')).toBeChecked();
  await expect(page.locator('#in-stats')).toBeChecked();

  const text = await outputText(page);
  expect(text).toContain('Crosstab: gender × satisfaction');
  expect(text).toContain('(blank)');
  expect(text).toContain('Chi-square =');
});
