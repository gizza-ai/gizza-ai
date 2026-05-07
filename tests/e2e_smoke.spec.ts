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

    // Open the settings dialog.
    await page.locator('#open-settings').click();
    await expect(page.locator('#settings')).toBeVisible();

    // Click "Load model". WebLLM will download the model weights on first run
    // (~1.2 GB for Qwen2.5-1.5B-Instruct-q4f32_1-MLC) so we allow 3 minutes.
    await page.locator('#load-model').click();
    await expect(page.locator('#load-model')).toHaveText('Ready', { timeout: 600_000 });

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

  test('web-fetch retrieves a same-origin fixture and the marker reaches the chat', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText(/gizza/i, { timeout: 30_000 });
    await expect(page.locator('#composer')).toBeVisible({ timeout: 30_000 });

    // Model weights are cached after the clock test in the same suite, but
    // allow up to 3 minutes for cold runs.
    await page.locator('#open-settings').click();
    await expect(page.locator('#settings')).toBeVisible();
    await page.locator('#load-model').click();
    await expect(page.locator('#load-model')).toHaveText('Ready', { timeout: 600_000 });
    await page.locator('#settings button[value="close"]').click();
    await expect(page.locator('#settings')).not.toBeVisible();

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
