import { test, expect } from './fixtures';

const table = `| name | role |
| --- | --- |
| Ada | Engineer |
| Bo | Designer |`;

test('markdown-table-to-csv page converts a Markdown table to CSV', async ({ page }) => {
  await page.goto('/tools/markdown-table-to-csv/');
  await page.fill('#in-markdown', table);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,role', { timeout: 15_000 });
  // Alignment row + padding stripped; no trailing newline.
  expect(await out.textContent()).toBe('name,role\nAda,Engineer\nBo,Designer');
});

test('markdown-table-to-csv page honors deep-linked delimiter, quoting, header, newline and BOM', async ({ page }) => {
  const md = '| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |';
  const qs =
    '?markdown=' + encodeURIComponent(md) +
    '&delimiter=semicolon' +
    '&header=false' +
    '&quote=all' +
    '&newline=crlf' +
    '&bom=true';
  await page.goto('/tools/markdown-table-to-csv/' + qs);

  // Enum <select> and checkbox controls reflect the deep-linked, non-default state.
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon', { timeout: 15_000 });
  await expect(page.locator('#in-quote')).toHaveValue('all');
  await expect(page.locator('#in-newline')).toHaveValue('crlf');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-bom')).toBeChecked();

  const out = page.locator('#tool-output');
  // header=false drops the "a|b" row; quote=all wraps every field; semicolon
  // delimiter; CRLF line ending between rows; UTF-8 BOM prepended.
  await expect(out).toContainText('"1";"2"', { timeout: 15_000 });
  expect(await out.textContent()).toBe('﻿"1";"2"\r\n"3";"4"');
});

test('markdown-table-to-csv page covers non-default checkbox and enum controls via the UI', async ({ page }) => {
  await page.goto('/tools/markdown-table-to-csv/');
  await page.fill('#in-markdown', table);
  await page.uncheck('#in-header');
  await page.selectOption('#in-quote', 'all');

  const out = page.locator('#tool-output');
  // Header row dropped, every remaining field wrapped in double quotes.
  await expect(out).toContainText('"Ada","Engineer"', { timeout: 15_000 });
  expect(await out.textContent()).toBe('"Ada","Engineer"\n"Bo","Designer"');
});

test('markdown-table-to-csv page reports a missing table clearly', async ({ page }) => {
  await page.goto('/tools/markdown-table-to-csv/');
  await page.fill('#in-markdown', 'just some prose, no pipes here');
  await expect(page.locator('#tool-output')).toContainText('no table rows found', { timeout: 15_000 });
});
