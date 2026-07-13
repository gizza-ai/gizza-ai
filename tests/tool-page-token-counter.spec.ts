import { test, expect } from './fixtures';

test('token-counter counts GPT-4o tokens and estimates cost', async ({ page }) => {
  await page.goto('/tools/token-counter/');
  await page.fill('#in-text', 'hello world');
  await page.selectOption('#in-model', 'gpt-4o');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Model: GPT-4o (OpenAI)', { timeout: 15000 });
  await expect(out).toContainText('Tokenizer: o200k_base — exact count');
  await expect(out).toContainText('Tokens: 2 (exact)');
  await expect(out).toContainText('Characters: 11');
  await expect(out).toContainText('Input:  $0.000005  (at $2.50 / 1M tokens)');
});

test('token-counter supports deep links and approximate Claude models', async ({ page }) => {
  const params = new URLSearchParams({ text: 'hello world', model: 'claude-opus-4.8' });
  await page.goto(`/tools/token-counter/?${params.toString()}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Model: Claude Opus 4.8 (Anthropic)', { timeout: 15000 });
  await expect(out).toContainText("Tokenizer: o200k_base — approximate (Claude Opus 4.8's tokenizer is proprietary)");
  await expect(out).toContainText('Tokens: 2 (approx)');
  await expect(out).toContainText('Output: $25.00 / 1M tokens (reference)');
});

test('token-counter handles cl100k models from the dropdown', async ({ page }) => {
  await page.goto('/tools/token-counter/');
  await page.fill('#in-text', 'Hello, world!');
  await page.selectOption('#in-model', 'gpt-3.5-turbo');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Model: GPT-3.5 Turbo (OpenAI)', { timeout: 15000 });
  await expect(out).toContainText('Tokenizer: cl100k_base — exact count');
  await expect(out).toContainText('Tokens: 4 (exact)');
});
