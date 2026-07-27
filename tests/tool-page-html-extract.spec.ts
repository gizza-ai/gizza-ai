import { test, expect } from './fixtures';

const sampleHtml = '<ul>\n  <li><a href="/one">First</a></li>\n  <li><a href="/two">Second</a></li>\n</ul>';

test('html-extract page extracts link text by default', async ({ page }) => {
  await page.goto('/tools/html-extract/');
  await page.fill('#in-html', sampleHtml);
  await page.fill('#in-selector', 'a');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('First', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual({
    count: 2,
    matches: ['First', 'Second'],
  });
});

test('html-extract deep link extracts href attributes', async ({ page }) => {
  const qs =
    '?html=' + encodeURIComponent(sampleHtml) +
    '&selector=' + encodeURIComponent('a') +
    '&extract=attr' +
    '&attr=href' +
    '&limit=1';
  await page.goto('/tools/html-extract/' + qs);

  await expect(page.locator('#in-extract')).toHaveValue('attr', { timeout: 15_000 });
  await expect(page.locator('#in-attr')).toHaveValue('href');
  await expect(page.locator('#in-limit')).toHaveValue('1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('/one', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual({
    count: 1,
    matches: ['/one'],
  });
});

test('html-extract supports inner/outer html and trim off', async ({ page }) => {
  await page.goto('/tools/html-extract/');
  await page.fill('#in-html', '<div><b>hi</b></div><p>  a   b  </p>');
  await page.fill('#in-selector', 'div');
  await page.selectOption('#in-extract', 'inner-html');

  let outText = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('<b>hi</b>');
  expect(JSON.parse(outText ?? '')).toEqual({ count: 1, matches: ['<b>hi</b>'] });

  await page.selectOption('#in-extract', 'outer-html');
  outText = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  expect(JSON.parse(outText ?? '')).toEqual({ count: 1, matches: ['<div><b>hi</b></div>'] });

  await page.fill('#in-selector', 'p');
  await page.selectOption('#in-extract', 'text');
  await page.uncheck('#in-trim');
  await expect(page.locator('#tool-output')).toContainText('a   b', { timeout: 15_000 });
  outText = await page.locator('#tool-output').textContent();
  expect(JSON.parse(outText ?? '')).toEqual({ count: 1, matches: ['  a   b  '] });
});
