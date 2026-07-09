// landing-chooser.spec.ts — chooser renders without SW/WebGPU and links out.
// The apex `/` is a static HTML page (no Service Worker, no runtime) that
// presents two buttons routing to /chat and /tools/. These tests assert the
// chooser structure without touching the LLM or WebGPU at all.
//
// The chooser test runs in a fresh ephemeral browser context with
// serviceWorkers: 'block' so a previously-registered SW from other test runs
// can never intercept GET / and return the full chat UI. This makes the
// assertion deterministic regardless of local SW lifecycle state.
import { test, expect, chromium } from '@playwright/test';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8001';

test('/ shows two buttons linking to /chat and /tools/', async () => {
  // Use a fresh ephemeral context with SWs blocked so stale SW state from
  // prior runs cannot intercept the request and serve the chat UI instead.
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    await page.goto('/');
    await expect(page.locator('.chooser__btn[href="/chat"]')).toBeVisible();
    await expect(page.locator('.chooser__btn[href="/tools/"]')).toBeVisible();
    await expect(page.locator('.chooser__buttons')).toBeVisible();
    // Confirm we are NOT on the chat UI (no #composer element)
    await expect(page.locator('#composer')).not.toBeAttached();
  } finally {
    await ctx.close();
    await browser.close();
  }
});

test('About resource link anchors to the on-page about section', async () => {
  // The mega-menu About entry must point at the chooser's own about section
  // (`/#about`) — a bare `/about` has no route anywhere: the SW would boot the
  // whole runtime just to render the router's JSON 404.
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    await page.goto('/');
    const about = page.locator('.mega-menu__resource-link', {
      has: page.locator('.mega-menu__resource-title', { hasText: 'About' }),
    });
    await expect(about).toHaveAttribute('href', '/#about');
    // ...and the anchor target must exist.
    await expect(page.locator('#about')).toBeAttached();
  } finally {
    await ctx.close();
    await browser.close();
  }
});

test('mascot idle video actually starts after the idle delay', async () => {
  // The chooser video is preload="none", so nothing loads until mascot.js
  // calls play(). Regression: showVideo() used to gate play() behind
  // `loadedmetadata`, which never fires under preload="none" — the mascot
  // swapped to an empty <video> that never fetched a byte. Assert the swap
  // happens AND playback was actually initiated (request issued + not paused).
  // Deliberately codec-independent: headless Chromium may lack H.264, so we
  // assert the fetch + play() call, not decoded frames.
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    const videoRequest = page.waitForRequest('**/gis_video_idle.mp4', { timeout: 20_000 });
    await page.goto('/');
    // Idle reveal fires after 10s of no typing/hover activity.
    await expect(page.locator('.brand-mascot')).toHaveAttribute('data-pose', 'video', {
      timeout: 15_000,
    });
    await videoRequest;
    await expect
      .poll(() => page.locator('.brand-video').evaluate((v: HTMLVideoElement) => !v.paused), {
        timeout: 10_000,
      })
      .toBe(true);
  } finally {
    await ctx.close();
    await browser.close();
  }
});

test('tools button reaches a working tool without the runtime', async () => {
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    await page.goto('/tools/');
    await expect(page).toHaveTitle(/Tools/);
  } finally {
    await ctx.close();
    await browser.close();
  }
});
