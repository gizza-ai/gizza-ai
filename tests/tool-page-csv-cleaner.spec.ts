import { test, expect } from './fixtures';

const MESSY = 'name , age\n Alice ,30\nBob,25\n,,\nBob,25\nCarol, 40 ';
const CLEANED = 'name,age\nAlice,30\nBob,25\nCarol,40\n';

test('csv-cleaner page trims, dedupes, and drops empty rows exactly', async ({ page }) => {
  await page.goto('/tools/csv-cleaner/');
  await page.fill('#in-data', MESSY);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Carol,40', { timeout: 15000 });
  expect(await out.textContent()).toBe(CLEANED);
});

test('csv-cleaner page fills blanks and honors non-default checkbox states', async ({ page }) => {
  await page.goto('/tools/csv-cleaner/');
  await page.fill('#in-data', 'a,b,c\n1,,3\n,5,');
  await page.selectOption('#in-empty_cells', 'fill');
  await page.fill('#in-fill_value', 'N/A');
  await page.uncheck('#in-dedupe');
  await page.uncheck('#in-drop_empty_rows');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('N/A,5,N/A', { timeout: 15000 });
  expect(await out.textContent()).toBe('a,b,c\n1,N/A,3\nN/A,5,N/A\n');
});

test('csv-cleaner page converts delimiter enum choices', async ({ page }) => {
  await page.goto('/tools/csv-cleaner/');
  await page.fill('#in-data', 'a,b\n1,2\n3,4');
  await page.selectOption('#in-output_delimiter', 'semicolon');
  let out = page.locator('#tool-output');
  await expect(out).toContainText('3;4', { timeout: 15000 });
  expect(await out.textContent()).toBe('a;b\n1;2\n3;4\n');

  await page.selectOption('#in-output_delimiter', 'pipe');
  await expect(out).toContainText('3|4', { timeout: 15000 });
  expect(await out.textContent()).toBe('a|b\n1|2\n3|4\n');
});

test('csv-cleaner page reads tab input and can treat row one as data', async ({ page }) => {
  await page.goto('/tools/csv-cleaner/');
  await page.fill('#in-data', 'x\ty\nx\ty\nz\tw');
  await page.fill('#in-delimiter', 'tab');
  await page.selectOption('#in-output_delimiter', 'comma');
  await page.uncheck('#in-header');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('z,w', { timeout: 15000 });
  expect(await out.textContent()).toBe('x,y\nz,w\n');
});

test('csv-cleaner page honors query-param deep link', async ({ page }) => {
  const data = encodeURIComponent(MESSY);
  await page.goto(`/tools/csv-cleaner/?data=${data}&header=true&trim=true&dedupe=true&drop_empty_rows=true&empty_cells=keep&output_delimiter=same&line_ending=lf`);
  await expect(page.locator('#in-data')).toHaveValue(MESSY);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Carol,40', { timeout: 15000 });
  expect(await out.textContent()).toBe(CLEANED);
});

test('csv-cleaner page download link serves exactly the visible output', async ({ page }) => {
  await page.goto('/tools/csv-cleaner/');
  const dl = page.locator('#tool-output-download');
  await expect(dl).toBeHidden();
  await page.fill('#in-data', MESSY);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Carol,40', { timeout: 15000 });
  await expect(dl).toBeVisible();
  expect(await dl.getAttribute('download')).toBe('csv-cleaner-output.txt');
  const blobText = await page.evaluate(async () => {
    const a = document.getElementById('tool-output-download') as HTMLAnchorElement;
    return (await fetch(a.href)).text();
  });
  expect(blobText).toBe(CLEANED);
});
