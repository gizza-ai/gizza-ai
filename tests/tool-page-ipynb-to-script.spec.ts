import { test, expect } from './fixtures';

const notebook = JSON.stringify({
  cells: [
    { cell_type: 'markdown', metadata: {}, source: ['# Demo\n', '\n', 'Explain the cell.'] },
    {
      cell_type: 'code',
      execution_count: 7,
      metadata: {},
      outputs: [{ output_type: 'stream', name: 'stdout', text: ['hello\n'] }],
      source: ['x = 1\n', "print('hello')"],
    },
  ],
  metadata: { language_info: { name: 'python' } },
  nbformat: 4,
  nbformat_minor: 5,
});

const defaultScript = "# # Demo\n#\n# Explain the cell.\n\nx = 1\nprint('hello')";

test('ipynb-to-script page extracts notebook code and drops outputs by default', async ({ page }) => {
  await page.goto('/tools/ipynb-to-script/');
  await page.fill('#in-notebook', notebook);

  const out = page.locator('#tool-output');
  await expect(out).toContainText(defaultScript, { timeout: 15_000 });
  await expect(out).not.toContainText('Output:');
  await expect(out).not.toContainText('execution_count');
});

test('ipynb-to-script page honors a deep-linked code-only script with cell markers', async ({ page }) => {
  const qs =
    '?notebook=' + encodeURIComponent(notebook) +
    '&output=script' +
    '&include_markdown=false' +
    '&include_outputs=false' +
    '&cell_markers=true';
  await page.goto('/tools/ipynb-to-script/' + qs);

  await expect(page.locator('#in-notebook')).toHaveValue(notebook, { timeout: 15_000 });
  await expect(page.locator('#in-include_markdown')).not.toBeChecked();
  await expect(page.locator('#in-cell_markers')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText("# %%\nx = 1\nprint('hello')", { timeout: 15_000 });
  await expect(out).not.toContainText('Explain the cell.');
});

test('ipynb-to-script page can export markdown with fenced code', async ({ page }) => {
  await page.goto('/tools/ipynb-to-script/');
  await page.fill('#in-notebook', notebook);
  await page.selectOption('#in-output', 'markdown');

  const out = page.locator('#tool-output');
  await expect(out).toContainText("# Demo\n\nExplain the cell.\n\n```python\nx = 1\nprint('hello')\n```", { timeout: 15_000 });
});

test('ipynb-to-script page reports invalid notebook JSON clearly', async ({ page }) => {
  await page.goto('/tools/ipynb-to-script/');
  await page.fill('#in-notebook', '{not json');
  await expect(page.locator('#tool-output')).toContainText('input is not valid JSON', { timeout: 15_000 });
});
