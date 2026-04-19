import { test, expect } from '@playwright/test';

test.describe('gizza-ai smoke', () => {
  test('page loads, model loads, clock prompt produces some response', async ({ page }) => {
    // First visit may register the Service Worker and reload the page.
    await page.goto('/');

    // Wait for the chat UI rendered by the gizza-ai/ui block (served via SW).
    // The SW intercepts /b/ui/ and the page transitions from the boot shell to
    // the full UI. h1 "gizza-ai" is present in both shells, but #composer only
    // appears in the rendered UI.
    await expect(page.locator('h1')).toContainText('gizza-ai', { timeout: 30_000 });
    await expect(page.locator('#composer')).toBeVisible({ timeout: 30_000 });

    // Open the settings dialog.
    await page.locator('#open-settings').click();
    await expect(page.locator('#settings')).toBeVisible();

    // Click "Load model". WebLLM will download the model weights on first run
    // (~1.2 GB for Qwen2.5-1.5B-Instruct-q4f32_1-MLC) so we allow 3 minutes.
    await page.locator('#load-model').click();
    await expect(page.locator('#load-model')).toHaveText('Ready', { timeout: 180_000 });

    // Close the settings dialog via the form[method=dialog] close button.
    await page.locator('#settings button[value="close"]').click();
    await expect(page.locator('#settings')).not.toBeVisible();

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
});
