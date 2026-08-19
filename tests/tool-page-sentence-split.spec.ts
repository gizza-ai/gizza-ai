import { test, expect } from './fixtures';

test('sentence-split page keeps abbreviations and decimals together', async ({
  page,
}) => {
  await page.goto('/tools/sentence-split/');

  await page.fill(
    '#in-text',
    'Dr. Green paid $99.99 for it. It arrived on Mar. 3 and works fine.',
  );
  await page.selectOption('#in-format', 'lines');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    'Dr. Green paid $99.99 for it.\nIt arrived on Mar. 3 and works fine.',
    { timeout: 15000 },
  );
});

test('sentence-split page supports numbered deep links and custom abbreviations', async ({
  page,
}) => {
  await page.goto(
    '/tools/sentence-split/?text=Ship%20it%20to%20Acme%20Corp.%20Then%20invoice%20Beta%20Ltd.%20Thanks.&format=numbered&newlines=paragraph&trim=true&min_chars=0&extra_abbreviations=Corp.%2C%20Ltd.',
  );

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '1. Ship it to Acme Corp. Then invoice Beta Ltd. Thanks.',
    { timeout: 15000 },
  );
  await expect(page.locator('#in-format')).toHaveValue('numbered');
  await expect(page.locator('#in-extra_abbreviations')).toHaveValue('Corp., Ltd.');
});

test('sentence-split page exposes newline mode and trim checkbox', async ({
  page,
}) => {
  await page.goto('/tools/sentence-split/');

  await page.fill('#in-text', 'First line\nsecond line\nThird line');
  await page.selectOption('#in-newlines', 'always');
  await page.uncheck('#in-trim');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('First line\nsecond line\nThird line', {
    timeout: 15000,
  });
  await expect(page.locator('#in-trim')).not.toBeChecked();
});
