import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('default markdown output covers every pair with a numbered # column', async ({ page }) => {
  await page.goto('/tools/pairwise-test-generator/');
  await page.fill('#in-parameters', 'A: 1, 2\nB: x, y');
  await expect(page.locator('#tool-output')).toContainText('| # | A | B |', { timeout: 15000 });
  expect(await output(page)).toBe(
    '| # | A | B |\n' +
      '| --- | --- | --- |\n' +
      '| 1 | 1 | x |\n' +
      '| 2 | 1 | y |\n' +
      '| 3 | 2 | x |\n' +
      '| 4 | 2 | y |',
  );
});

test('deep-link pre-fills parameters and renders CSV', async ({ page }) => {
  const parameters = encodeURIComponent('A: 1, 2\nB: x, y');
  await page.goto(`/tools/pairwise-test-generator/?parameters=${parameters}&output_format=csv&include_index=true`);
  await expect(page.locator('#tool-output')).toContainText('#,A,B', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('csv');
  expect(await output(page)).toBe('#,A,B\n1,1,x\n2,1,y\n3,2,x\n4,2,y');
});

test('ascii format with the # column unchecked drops the index', async ({ page }) => {
  await page.goto('/tools/pairwise-test-generator/');
  await page.fill('#in-parameters', 'A: 1, 2\nB: x, y');
  await page.selectOption('#in-output_format', 'ascii');
  await page.uncheck('#in-include_index');
  await expect(page.locator('#tool-output')).toContainText('+---+---+', { timeout: 15000 });
  await expect(page.locator('#in-include_index')).not.toBeChecked();
  expect(await output(page)).toBe(
    '+---+---+\n' +
      '| A | B |\n' +
      '+---+---+\n' +
      '| 1 | x |\n' +
      '| 1 | y |\n' +
      '| 2 | x |\n' +
      '| 2 | y |\n' +
      '+---+---+',
  );
});

test('json format emits an array of objects with a numeric index', async ({ page }) => {
  await page.goto('/tools/pairwise-test-generator/');
  await page.fill('#in-parameters', 'A: 1, 2\nB: x, y');
  await page.selectOption('#in-output_format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"#": 1', { timeout: 15000 });
  expect(await output(page)).toBe(
    '[\n' +
      '  {"#": 1, "A": "1", "B": "x"},\n' +
      '  {"#": 2, "A": "1", "B": "y"},\n' +
      '  {"#": 3, "A": "2", "B": "x"},\n' +
      '  {"#": 4, "A": "2", "B": "y"}\n' +
      ']',
  );
});
