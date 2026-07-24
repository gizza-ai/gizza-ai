import { test, expect } from './fixtures';

const JS_SAMPLE = `=== src/app.js ===
import { greet } from './util/greet';
import React from 'react';
const fs = require('fs');

=== src/util/greet.js ===
export const greet = () => 'hi';`;

const PY_CYCLE = `=== pkg/a.py ===
from .b import thing
import os

=== pkg/b.py ===
from pkg.a import other`;

test('import-graph-extractor page reports JS internal and external dependencies', async ({ page }) => {
  await page.goto('/tools/import-graph-extractor/');
  await page.fill('#in-input', JS_SAMPLE);
  await page.selectOption('#in-language', 'auto');
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('src/app.js -> src/util/greet.js', { timeout: 15000 });
  await expect(out).toContainText('react [package]');
  await expect(out).toContainText('fs [stdlib]');
  await expect(out).toContainText('src/util/greet.js <- src/app.js');
});

test('import-graph-extractor deep-link prefills Python cycle and renders Mermaid', async ({ page }) => {
  await page.goto(
    '/tools/import-graph-extractor/?' +
      new URLSearchParams({
        input: PY_CYCLE,
        language: 'python',
        format: 'mermaid',
        include_external: 'true',
        detect_cycles: 'true',
      }).toString()
  );

  await expect(page.locator('#in-input')).toHaveValue(PY_CYCLE, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('graph LR', { timeout: 15000 });
  await expect(out).toContainText('-->');
  await expect(out).toContainText('classDef cycle');
});

test('import-graph-extractor JSON output and include_external checkbox off', async ({ page }) => {
  await page.goto('/tools/import-graph-extractor/');
  await page.fill('#in-input', JS_SAMPLE);
  await page.selectOption('#in-format', 'json');
  await page.uncheck('#in-include_external');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"from": "src/app.js"', { timeout: 15000 });
  await expect(out).toContainText('"to": "src/util/greet.js"');
  const text = (await out.textContent()) ?? '';
  expect(text).not.toContain('react');
  expect(text).not.toContain('fs');
});

test('import-graph-extractor DOT enum output includes Graphviz edge', async ({ page }) => {
  await page.goto('/tools/import-graph-extractor/');
  await page.fill('#in-input', `=== a.js ===\nimport './b';\n=== b.js ===\nexport const b = 1;`);
  await page.selectOption('#in-format', 'dot');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('digraph imports {', { timeout: 15000 });
  await expect(out).toContainText('"a.js" -> "b.js"');
});
