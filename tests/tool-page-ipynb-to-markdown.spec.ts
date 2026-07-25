import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const NOTEBOOK = JSON.stringify({
  cells: [
    { cell_type: 'markdown', metadata: {}, source: ['# Demo\n', 'Some notes'] },
    {
      cell_type: 'code',
      execution_count: 1,
      metadata: {},
      source: ['print(2 + 2)'],
      outputs: [{ output_type: 'stream', name: 'stdout', text: ['4\n'] }],
    },
  ],
  metadata: { language_info: { name: 'python' } },
  nbformat: 4,
  nbformat_minor: 5,
});

const FULL_EXPECTED = '# Demo\nSome notes\n\n```python\nprint(2 + 2)\n```\n\n```\n4\n```';

test('ipynb-to-markdown page — renders markdown, code, and stdout output', async ({ page }) => {
  await page.goto('/tools/ipynb-to-markdown/');
  await page.fill('#in-notebook', NOTEBOOK);
  await expect(page.locator('#tool-output')).toContainText('print(2 + 2)', { timeout: 15000 });
  expect(await outputText(page)).toBe(FULL_EXPECTED);
});

test('ipynb-to-markdown page — no-input export keeps outputs', async ({ page }) => {
  await page.goto('/tools/ipynb-to-markdown/');
  await page.fill('#in-notebook', NOTEBOOK);
  await page.uncheck('#in-include_code');
  await expect(page.locator('#tool-output')).toContainText('4', { timeout: 15000 });
  expect(await outputText(page)).toBe('# Demo\nSome notes\n\n```\n4\n```');
});

test('ipynb-to-markdown page — image modes and prompts', async ({ page }) => {
  const imageNotebook = JSON.stringify({
    cells: [
      {
        cell_type: 'code',
        execution_count: 3,
        source: ['plot()'],
        outputs: [{ output_type: 'display_data', data: { 'image/png': 'iVBORw0KGgo=' }, metadata: {} }],
      },
    ],
    metadata: { language_info: { name: 'python' } },
    nbformat: 4,
    nbformat_minor: 5,
  });
  await page.goto('/tools/ipynb-to-markdown/');
  await page.fill('#in-notebook', imageNotebook);
  await page.check('#in-show_prompts');
  await page.selectOption('#in-image_mode', 'placeholder');
  await expect(page.locator('#tool-output')).toContainText('**In [3]:**', { timeout: 15000 });
  expect(await outputText(page)).toBe('**In [3]:**\n```python\nplot()\n```\n\n**Out [3]:**\n*[image output]*');
});

test('ipynb-to-markdown page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/ipynb-to-markdown/?notebook=' +
      encodeURIComponent(NOTEBOOK) +
      '&include_code=true&include_outputs=true&include_markdown=true&show_prompts=false&image_mode=embed',
  );
  await expect(page.locator('#in-notebook')).toHaveValue(NOTEBOOK, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('print(2 + 2)', { timeout: 15000 });
  expect(await outputText(page)).toBe(FULL_EXPECTED);
});
