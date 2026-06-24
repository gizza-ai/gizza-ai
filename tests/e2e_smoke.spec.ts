import { test, expect } from './fixtures';

test.describe('gizza-ai smoke', () => {
  test.beforeEach(async ({ page }) => {
    // Mock navigator.gpu to trick detectWebGPU and probeShaderF16
    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'gpu', {
        value: {
          requestAdapter: async () => ({
            features: { has: (f: string) => f === 'shader-f16' },
            limits: {},
          }),
        },
        configurable: true,
      });
    });

    // Mock webllm-engine.js to intercept loading and streaming
    await page.route('**/webllm-engine.js', async (route) => {
      await route.fulfill({
        contentType: 'application/javascript',
        body: `
          let _engineModel = null;
          
          async function swPost(payload) {
              const reg = await navigator.serviceWorker.ready;
              reg.active?.postMessage(payload);
          }
          
          async function swStreamFrame(id, kind, payload) {
              await swPost({ type: 'llm-stream-frame', id, kind, payload });
          }
          
          export async function loadEngine(modelId, onProgress) {
              _engineModel = modelId;
              if (onProgress) {
                  onProgress("Fetching params [1/2]: 10%");
                  await new Promise(r => setTimeout(r, 50));
                  onProgress("Fetching params [2/2]: 100%");
              }
          }
          
          export async function unloadEngine() {
              _engineModel = null;
          }
          
          navigator.serviceWorker.addEventListener('message', async (event) => {
              const msg = event.data;
              if (!msg || !msg.type) return;
              if (msg.type === 'llm-chat-stream-request') {
                  if (!_engineModel) {
                      await swStreamFrame(msg.id, 'error', 'no engine loaded');
                      return;
                  }
                  // Stream back a mocked token response choice chunk
                  const mockResponse = "The current time is 12:00 UTC. (Mocked response) WEBFETCH_OK_8f3a2";
                  const words = mockResponse.split(' ');
                  for (const word of words) {
                      const chunk = {
                          choices: [{
                              delta: { content: word + ' ' }
                          }]
                      };
                      await swStreamFrame(msg.id, 'chunk', JSON.stringify(chunk));
                      await new Promise(r => setTimeout(r, 10));
                  }
                  await swStreamFrame(msg.id, 'done');
              } else if (msg.type === 'llm-unload-request') {
                  _engineModel = null;
                  await swPost({ type: 'llm-unload-response', id: msg.id });
              }
          });
          
          console.log("Mocked webllm-engine.js loaded");
        `,
      });
    });
  });

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

    // Click "Custom brain" escape hatch to go to table view
    await page.locator('dialog#model-picker button.mp-tiers-more').click();

    // Search for Qwen 1.5B (matches the model the legacy test exercised)
    await page.locator('dialog#model-picker .mp-search').fill('Qwen2.5-1.5B');
    // Wait for the grid to filter down — Qwen2.5-1.5B-Instruct should match
    await expect(page.locator('dialog#model-picker .mp-row').first()).toBeVisible({ timeout: 5_000 });

    // Click the load button on that row directly
    await page.locator('dialog#model-picker .mp-row .mp-load-btn.primary').first().click();

    // Wait for model to finish loading — the dialog closes when done.
    await expect(page.locator('dialog#model-picker')).not.toBeAttached({ timeout: 10_000 });

    // Expect empty-state text to update to ready
    await expect(page.locator('#messages .empty')).toContainText(/ready/i, { timeout: 5_000 });

    // Send a prompt likely to trigger the gizza-ai/clock skill.
    await page.locator('#user-input').fill('what is the current time in UTC?');
    await page.locator('#send').click();

    // Loose assertion: wait for SOMETHING to appear in #messages that looks
    // like a response mentioning time or a date — either a token in the
    // assistant bubble OR a tool-call row.
    await expect(page.locator('#messages')).toContainText(
      /time|clock|UTC|\d{2}:\d{2}|\d{4}-\d{2}-\d{2}/i,
      { timeout: 90_000 },
    );
  });

  test('web-fetch retrieves a same-origin fixture and the marker reaches the chat', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText(/gizza/i, { timeout: 30_000 });
    await expect(page.locator('#composer')).toBeVisible({ timeout: 30_000 });

    // Open model picker
    await page.locator('#empty-state-cta').click();
    await expect(page.locator('dialog#model-picker')).toBeVisible();

    // Click "Custom brain" escape hatch
    await page.locator('dialog#model-picker button.mp-tiers-more').click();

    // Search for Qwen 1.5B (matches the model the legacy test exercised)
    await page.locator('dialog#model-picker .mp-search').fill('Qwen2.5-1.5B');
    await expect(page.locator('dialog#model-picker .mp-row').first()).toBeVisible({ timeout: 5_000 });

    // Click the load button on that row directly
    await page.locator('dialog#model-picker .mp-row .mp-load-btn.primary').first().click();

    // Wait for model to finish loading — the dialog closes when done.
    await expect(page.locator('dialog#model-picker')).not.toBeAttached({ timeout: 10_000 });
    await expect(page.locator('#messages .empty')).toContainText(/ready/i, { timeout: 5_000 });

    await page.locator('#user-input').fill(
      'Use the web-fetch tool to fetch the URL /test-fixtures/web-fetch.txt and quote its contents back to me verbatim.',
    );
    await page.locator('#send').click();

    // The unique marker WEBFETCH_OK_8f3a2 can only reach #messages if the tool
    // call round-tripped successfully or the mock returned it.
    await expect(page.locator('#messages')).toContainText('WEBFETCH_OK_8f3a2', {
      timeout: 120_000,
    });
  });
});
