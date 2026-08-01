import { test, expect } from './fixtures';

const scoreCsv = 'name,score\nAlice,12\nBob,55\nCarol,88\nDan,73';

test('data-bin page bins a column into equal-width quartiles with range labels', async ({ page }) => {
  await page.goto('/tools/data-bin/');
  await page.fill('#in-input', scoreCsv);
  await page.fill('#in-column', 'score');

  // Range labels containing a comma are quoted by the CSV writer.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('(69, 88]', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'name,score,score_bin\nAlice,12,"[12, 31]"\nBob,55,"(50, 69]"\nCarol,88,"(69, 88]"\nDan,73,"(69, 88]"\n',
  );
});

test('data-bin deep link runs quantile terciles with custom labels', async ({ page }) => {
  const csv = 'name,score\nAlice,12\nBob,55\nCarol,88\nDan,73\nEve,40\nFrank,95';
  const qs =
    '?input=' + encodeURIComponent(csv) +
    '&method=quantile' +
    '&column=score' +
    '&bins=3' +
    '&labels=' + encodeURIComponent('low,mid,high');
  await page.goto('/tools/data-bin/' + qs);

  await expect(page.locator('#in-method')).toHaveValue('quantile', { timeout: 15_000 });
  await expect(page.locator('#in-column')).toHaveValue('score');
  await expect(page.locator('#in-bins')).toHaveValue('3');
  await expect(page.locator('#in-labels')).toHaveValue('low,mid,high');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Alice,12,low', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'name,score,score_bin\nAlice,12,low\nBob,55,mid\nCarol,88,high\nDan,73,mid\nEve,40,low\nFrank,95,high\n',
  );
});

test('data-bin deep link supports custom edges with replace output', async ({ page }) => {
  const csv = 'name,age\nA,10\nB,40\nC,80';
  const qs =
    '?input=' + encodeURIComponent(csv) +
    '&method=custom' +
    '&column=age' +
    '&edges=' + encodeURIComponent('0,18,65,120') +
    '&labels=' + encodeURIComponent('child,adult,senior') +
    '&output=replace';
  await page.goto('/tools/data-bin/' + qs);

  await expect(page.locator('#in-method')).toHaveValue('custom', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('replace');
  await expect(page.locator('#in-edges')).toHaveValue('0,18,65,120');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('A,child', { timeout: 15_000 });
  expect(await out.textContent()).toBe('name,age\nA,child\nB,adult\nC,senior\n');
});

test('data-bin deep link covers index labels, left-closed intervals, and semicolon delimiter', async ({ page }) => {
  const csv = 'id;score\na;0\nb;50\nc;100';
  const qs =
    '?input=' + encodeURIComponent(csv) +
    '&method=equal_width' +
    '&column=score' +
    '&bins=2' +
    '&delimiter=semicolon' +
    '&label_style=index' +
    '&right=false';
  await page.goto('/tools/data-bin/' + qs);

  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon', { timeout: 15_000 });
  await expect(page.locator('#in-label_style')).toHaveValue('index');
  await expect(page.locator('#in-right')).not.toBeChecked();

  // Left-closed [0,50) / [50,100] → 50 lands in the upper (second) bucket.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('b;50;2', { timeout: 15_000 });
  expect(await out.textContent()).toBe('id;score;score_bin\na;0;1\nb;50;2\nc;100;2\n');
});
