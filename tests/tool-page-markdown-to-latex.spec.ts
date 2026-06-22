import { test, expect } from './fixtures';

test('markdown-to-latex page converts headings and inline formatting', async ({ page }) => {
  await page.goto('/tools/markdown-to-latex/');

  await page.fill('#in-markdown', '# Title\n\nSome **bold** and *italic* and `code`.');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\section{Title}', { timeout: 15000 });
  await expect(out).toContainText('\\textbf{bold}');
  await expect(out).toContainText('\\textit{italic}');
  await expect(out).toContainText('\\texttt{code}');
});

test('markdown-to-latex full_document checkbox wraps a standalone document', async ({ page }) => {
  await page.goto('/tools/markdown-to-latex/');
  await page.fill('#in-markdown', '# Hi');
  await page.check('#in-full_document');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\documentclass{article}', { timeout: 15000 });
  await expect(out).toContainText('\\begin{document}');
  await expect(out).toContainText('\\end{document}');
});

test('markdown-to-latex heading_offset demotes headings', async ({ page }) => {
  await page.goto('/tools/markdown-to-latex/');
  await page.fill('#in-markdown', '# Top');
  await page.fill('#in-heading_offset', '2');
  await expect(page.locator('#tool-output')).toContainText('\\subsubsection{Top}', {
    timeout: 15000,
  });
});

test('markdown-to-latex converts strikethrough, footnotes, and setext headings', async ({ page }) => {
  await page.goto('/tools/markdown-to-latex/');
  await page.fill(
    '#in-markdown',
    'Use ~~old~~ new and a note[^1].\n\n[^1]: Footnote text.\n\nTitle\n=====',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\sout{old}', { timeout: 15000 });
  await expect(out).toContainText('\\footnote{Footnote text.}');
  await expect(out).toContainText('\\section{Title}');
});

test('markdown-to-latex query param pre-fills and computes', async ({ page }) => {
  await page.goto('/tools/markdown-to-latex/?markdown=' + encodeURIComponent('## Deep'));
  await expect(page.locator('#tool-output')).toContainText('\\subsection{Deep}', {
    timeout: 15000,
  });
});
