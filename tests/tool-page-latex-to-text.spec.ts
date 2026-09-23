import { test, expect } from './fixtures';

const SOURCE = String.raw`\documentclass{article}
\title{On Ducks}
\begin{document}
\section{Introduction}
The \textbf{mallard} is a common duck~\cite{smith2024}.
It swims at $v = 3$ m/s.
\end{document}`;

async function setText(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

async function output(page: import('@playwright/test').Page) {
  await expect(page.locator('#tool-output')).not.toHaveText('', { timeout: 15_000 });
  return (await page.locator('#tool-output').textContent()) || '';
}

test('latex-to-text page strips commands into exact default prose', async ({ page }) => {
  await page.goto('/tools/latex-to-text/');
  await setText(page, SOURCE);
  const text = await output(page);
  expect(text).toBe('Introduction\n\nThe mallard is a common duck. It swims at m/s.');
});

test('latex-to-text deep link keeps math placeholders and citation keys', async ({ page }) => {
  await page.goto('/tools/latex-to-text/?input=As%20shown%20%5Ccite%7Bknuth1984%7D%20%24x%2B1%24.&math=placeholder&citations=keys&unicode=true&body_only=true&line_breaks=paragraphs');
  await expect(page.locator('#in-math')).toHaveValue('placeholder', { timeout: 15_000 });
  await expect(page.locator('#in-citations')).toHaveValue('keys');
  const text = await output(page);
  expect(text).toBe('As shown knuth1984 [math].');
});

test('latex-to-text supports all advertised math modes and a non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/latex-to-text/');
  await setText(page, String.raw`Caf\'e $E=mc^2$`);

  await page.selectOption('#in-math', 'remove');
  expect(await output(page)).toBe('Café');

  await page.selectOption('#in-math', 'placeholder');
  expect(await output(page)).toBe('Café [math]');

  await page.selectOption('#in-math', 'keep');
  expect(await output(page)).toBe(String.raw`Café $E=mc^2$`);

  await page.uncheck('#in-unicode');
  expect(await output(page)).toBe(String.raw`Cafe $E=mc^2$`);
});

test('latex-to-text preserves source line breaks and accepts the exact cap boundary', async ({ page }) => {
  await page.goto('/tools/latex-to-text/');
  await setText(page, 'One line\nsecond line\n\nNew paragraph');
  await page.selectOption('#in-line_breaks', 'source');
  expect(await output(page)).toBe('One line\nsecond line\n\nNew paragraph');

  await setText(page, 'a'.repeat(1_000_000));
  await page.selectOption('#in-line_breaks', 'paragraphs');
  expect((await output(page)).length).toBe(1_000_000);
});

test('latex-to-text ships preset chips and a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/latex-to-text/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await page.locator('.tool-example-chip', { hasText: 'Keep citation keys' }).click();
  await expect(page.locator('#in-citations')).toHaveValue('keys');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(String.raw`gizza tool latex-to-text '\section{Introduction}
The \textbf{mallard} swims at $v = 3$ m/s. See \cite{smith2024}.'`);
});
