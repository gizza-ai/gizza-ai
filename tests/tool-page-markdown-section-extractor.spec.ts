import { test, expect } from './fixtures';

const DOC = `# Title

Intro.

## Install

Run it.

### Linux

apt install foo

## Usage

use it`;

test('markdown-section-extractor page extracts a section with subsections', async ({ page }) => {
  await page.goto('/tools/markdown-section-extractor/');
  await page.fill('#in-markdown', DOC);
  await page.fill('#in-heading', 'Install');
  const out = page.locator('#tool-output');
  // Default: include the heading + every nested subsection, stop before the next ##.
  await expect(out).toContainText('## Install', { timeout: 15000 });
  await expect(out).toContainText('Run it.');
  await expect(out).toContainText('### Linux');
  await expect(out).toContainText('apt install foo');
  await expect(out).not.toContainText('Usage');
});

test('markdown-section-extractor exclude-subsections checkbox stops at the first deeper heading', async ({ page }) => {
  await page.goto('/tools/markdown-section-extractor/');
  await page.fill('#in-markdown', DOC);
  await page.fill('#in-heading', 'Install');
  await page.uncheck('#in-include_subsections');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('## Install', { timeout: 15000 });
  await expect(out).toContainText('Run it.');
  await expect(out).not.toContainText('Linux');
});

test('markdown-section-extractor exclude-heading checkbox returns body only', async ({ page }) => {
  await page.goto('/tools/markdown-section-extractor/');
  await page.fill('#in-markdown', DOC);
  await page.fill('#in-heading', 'Usage');
  await page.uncheck('#in-include_heading');
  await expect(page.locator('#tool-output')).toHaveText('use it', { timeout: 15000 });
});

test('markdown-section-extractor contains match mode finds a substring heading', async ({ page }) => {
  await page.goto('/tools/markdown-section-extractor/');
  await page.fill('#in-markdown', DOC);
  await page.fill('#in-heading', 'linux');
  await page.selectOption('#in-match_mode', 'contains');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('### Linux', { timeout: 15000 });
  await expect(out).toContainText('apt install foo');
});

test('markdown-section-extractor query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/markdown-section-extractor/?markdown=' +
      encodeURIComponent(DOC) +
      '&heading=Usage',
  );
  await expect(page.locator('#in-heading')).toHaveValue('Usage', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('use it', { timeout: 15000 });
});
