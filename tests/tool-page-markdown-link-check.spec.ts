import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const BROKEN = `Issues
  3:1  error  ML007  broken anchor '#setup' — no heading in this document produces that id

1 error(s), 0 warning(s) in 1 link(s) checked.`;

const FULL = `Issues
  3:5  error  ML007  broken anchor '#setup' — no heading in this document produces that id
  6:1  error  ML005  duplicate reference definition [ref] — the first definition wins, this one is ignored

2 error(s), 0 warning(s) in 3 link(s) checked.`;

test('markdown-link-check page reports a broken in-document anchor exactly', async ({ page }) => {
  await page.goto('/tools/markdown-link-check/');
  await setField(page, '#in-markdown', '# Install\n\n[jump](#setup)\n');

  await expect(page.locator('#tool-output')).toContainText('ML007', { timeout: 15_000 });
  expect(await outputText(page)).toBe(BROKEN);
});

test('markdown-link-check deep link filters anchors and emits JSON', async ({ page }) => {
  const params = new URLSearchParams({
    markdown: '# Install\n\n[jump](#setup)\n',
    link_kind: 'anchor',
    report_format: 'json',
    show_ok: 'false',
    check_anchors: 'true',
    flag_insecure: 'false',
  });
  await page.goto(`/tools/markdown-link-check/?${params.toString()}`);

  await expect(page.locator('#in-link_kind')).toHaveValue('anchor', { timeout: 15_000 });
  await expect(page.locator('#in-report_format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"rule": "ML007"', {
    timeout: 15_000,
  });
  const body = JSON.parse(await outputText(page));
  expect(body.checked).toBe(1);
  expect(body.errors).toBe(1);
  expect(body.links[0]).toMatchObject({ kind: 'anchor', status: 'error', target: '#setup' });
});

test('markdown-link-check can list passing links', async ({ page }) => {
  await page.goto('/tools/markdown-link-check/');
  await setField(page, '#in-markdown', '# Install\n\n[jump](#install)\n');
  await page.check('#in-show_ok');

  await expect(page.locator('#tool-output')).toContainText('anchor  ok', { timeout: 15_000 });
  expect(await outputText(page)).toBe(
    `Links
  3:1  anchor  ok  [jump] -> #install

No link problems found — 1 link(s) checked.`,
  );
});

test('markdown-link-check shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/markdown-link-check/');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool markdown-link-check');
  expect(cli).toContain('[setup](#setup)');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');

  await setField(
    page,
    '#in-markdown',
    '# Install\n\nSee [setup](#setup), [site](https://example.com), and [missing][ref].\n\n[ref]: https://example.com/one\n[ref]: https://example.com/two',
  );
  await expect(page.locator('#tool-output')).toContainText('duplicate reference definition', {
    timeout: 15_000,
  });
  expect(await outputText(page)).toBe(FULL);
});
