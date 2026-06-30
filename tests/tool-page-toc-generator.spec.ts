import { test, expect } from './fixtures';

async function fillText(page: any, selector: string, value: string) {
  await page.$eval(
    selector,
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('toc-generator page builds a Markdown table of contents', async ({ page }) => {
  await page.goto('/tools/toc-generator/');
  await fillText(page, '#in-document', '# Title\n## Setup\n## Usage\n### Details');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('- [Title](#title)', { timeout: 15000 });
  await expect(out).toContainText('  - [Setup](#setup)');
  await expect(out).toContainText('    - [Details](#details)');
});

test('toc-generator page supports HTML input and HTML output', async ({ page }) => {
  await page.goto('/tools/toc-generator/');
  await page.selectOption('#in-input_format', 'html');
  await page.selectOption('#in-output_format', 'html');
  await fillText(page, '#in-document', '<h1 id="intro">Intro</h1><h2>Next Step</h2>');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<ul>', { timeout: 15000 });
  await expect(out).toContainText('<a href="#intro">Intro</a>');
  await expect(out).toContainText('<a href="#next-step">Next Step</a>');
});

test('toc-generator query-param deep-link prefills filters and computes', async ({ page }) => {
  const doc = '# Title\n## Setup\n### Details';
  await page.goto(
    '/tools/toc-generator/?document=' +
      encodeURIComponent(doc) +
      '&input_format=markdown&output_format=markdown&min_level=2&max_level=3&ordered=true',
  );
  await expect(page.locator('#in-document')).toHaveValue(doc, { timeout: 15000 });
  await expect(page.locator('#in-min_level')).toHaveValue('2');
  await expect(page.locator('#in-ordered')).toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).not.toContainText('[Title](#title)');
  await expect(out).toContainText('1. [Setup](#setup)', { timeout: 15000 });
  await expect(out).toContainText('  1. [Details](#details)');
});
