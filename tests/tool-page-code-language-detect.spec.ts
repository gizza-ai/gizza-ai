import { test, expect } from './fixtures';

const rustSnippet = `use std::collections::HashMap;

pub fn tally(words: &[&str]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for w in words {
        *counts.entry(w.to_string()).or_insert(0) += 1;
    }
    counts
}`;

test('code-language-detect page reports Rust with evidence', async ({ page }) => {
  await page.goto('/tools/code-language-detect/');
  await page.fill('#in-code', rustSnippet);
  await page.fill('#in-filename', 'main.rs');
  await page.selectOption('#in-output', 'report');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Detected language: Rust (rust)', { timeout: 15000 });
  await expect(out).toContainText('Confidence:');
  await expect(out).toContainText('Evidence for Rust:');
  await expect(out).toContainText('filename extension `.rs`');
});

test('code-language-detect page supports deep links and JSON output', async ({ page }) => {
  await page.goto('/tools/code-language-detect/?code=interface%20User%20%7B%0A%20%20name%3A%20string%3B%0A%7D&filename=user.ts&output=json&top_k=2&common_only=false&explain=true');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"language": "typescript"', { timeout: 15000 });
  await expect(out).toContainText('"language_name": "TypeScript"');
  await expect(out).toContainText('"candidates"');
});

test('code-language-detect page exercises filters, checkbox off path and language-only output', async ({ page }) => {
  await page.goto('/tools/code-language-detect/');
  await page.fill('#in-code', 'print("hello")');
  await page.fill('#in-candidates', 'python,javascript,ruby');
  await page.check('#in-common_only');
  await page.uncheck('#in-explain');
  await page.fill('#in-top_k', '0');
  await page.selectOption('#in-output', 'language');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(/^(python|javascript|ruby)$/, { timeout: 15000 });
});
