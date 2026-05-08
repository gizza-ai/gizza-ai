import { test, expect } from './fixtures';

test.describe('gizza-ai smoke', () => {
  test('page loads, model loads, clock prompt produces some response', async ({ page }) => {
    // First visit may register the Service Worker and reload the page.
    await page.goto('/');

    // Wait for the chat UI rendered by the gizza-ai/ui block (served via SW).
    // The SW intercepts /b/ui/ and the page transitions from the boot shell to
    // the full UI. The h1 reads "gizza.ai" in the rendered UI; #composer only
    // appears once the SW takes over.
    await expect(page.locator('h1')).toContainText(/gizza/i, { timeout: 30_000 });
    await expect(page.locator('#composer')).toBeVisible({ timeout: 30_000 });

    // New picker flow: empty-state CTA opens the picker overlay
    await page.locator('#empty-state-cta').click();
    await expect(page.locator('dialog#model-picker')).toBeVisible();

    // Search for Qwen 1.5B (matches the model the legacy test exercised)
    await page.locator('dialog#model-picker .mp-search').fill('Qwen2.5-1.5B');
    // Wait for the grid to filter down — Qwen2.5-1.5B-Instruct should match
    await expect(page.locator('dialog#model-picker .mp-card').first()).toBeVisible({ timeout: 5_000 });

    // Click the card → footer enables, default quality selected (middle variant)
    await page.locator('dialog#model-picker .mp-card').first().click();
    await expect(page.locator('dialog#model-picker .mp-card.selected')).toHaveCount(1);
    await expect(page.locator('dialog#model-picker .mp-footer button.primary')).toBeEnabled();

    // Load → overlay closes, progress card appears in chat area
    await page.locator('dialog#model-picker .mp-footer button.primary').click();
    await expect(page.locator('dialog#model-picker')).not.toBeAttached({ timeout: 5_000 });

    // Wait for model to finish loading — progress card disappears when ready.
    // 10 minutes upper bound (cold first-run download is ~1.6 GB on Qwen2.5-1.5B).
    await expect(page.locator('.progress-card')).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('.progress-card')).not.toBeVisible({ timeout: 600_000 });

    // Send a prompt likely to trigger the gizza-ai/clock skill.
    await page.locator('#user-input').fill('what is the current time in UTC?');
    await page.locator('#send').click();

    // Loose assertion: wait for SOMETHING to appear in #messages that looks
    // like a response mentioning time or a date — either a token in the
    // assistant bubble OR a tool-call row. WebLLM output is non-deterministic;
    // the test is a smoke — primarily verifying end-to-end plumbing works.
    await expect(page.locator('#messages')).toContainText(
      /time|clock|UTC|\d{2}:\d{2}|\d{4}-\d{2}-\d{2}/i,
      { timeout: 90_000 },
    );
  });

  test('web-fetch retrieves a same-origin fixture and the marker reaches the chat', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText(/gizza/i, { timeout: 30_000 });
    await expect(page.locator('#composer')).toBeVisible({ timeout: 30_000 });

    // Model weights are cached after the clock test in the same suite, but
    // allow up to 10 minutes for cold runs.
    // New picker flow: empty-state CTA opens the picker overlay
    await page.locator('#empty-state-cta').click();
    await expect(page.locator('dialog#model-picker')).toBeVisible();

    // Search for Qwen 1.5B (matches the model the legacy test exercised)
    await page.locator('dialog#model-picker .mp-search').fill('Qwen2.5-1.5B');
    // Wait for the grid to filter down — Qwen2.5-1.5B-Instruct should match
    await expect(page.locator('dialog#model-picker .mp-card').first()).toBeVisible({ timeout: 5_000 });

    // Click the card → footer enables, default quality selected (middle variant)
    await page.locator('dialog#model-picker .mp-card').first().click();
    await expect(page.locator('dialog#model-picker .mp-card.selected')).toHaveCount(1);
    await expect(page.locator('dialog#model-picker .mp-footer button.primary')).toBeEnabled();

    // Load → overlay closes, progress card appears in chat area
    await page.locator('dialog#model-picker .mp-footer button.primary').click();
    await expect(page.locator('dialog#model-picker')).not.toBeAttached({ timeout: 5_000 });

    // Wait for model to finish loading — progress card disappears when ready.
    // 10 minutes upper bound (cold first-run download is ~1.6 GB on Qwen2.5-1.5B).
    await expect(page.locator('.progress-card')).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('.progress-card')).not.toBeVisible({ timeout: 600_000 });

    await page.locator('#user-input').fill(
      'Use the web-fetch tool to fetch the URL /test-fixtures/web-fetch.txt and quote its contents back to me verbatim.',
    );
    await page.locator('#send').click();

    // The unique marker WEBFETCH_OK_8f3a2 can only reach #messages if the tool
    // call round-tripped successfully (skill → call_block → network block →
    // BrowserNetworkService → fetch → back through call_block → tool response → LLM).
    await expect(page.locator('#messages')).toContainText('WEBFETCH_OK_8f3a2', {
      timeout: 120_000,
    });
  });
});
