import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = (name: string) => path.resolve(__dirname, 'fixtures', name);

async function waitForSplit(page: import('@playwright/test').Page) {
  await expect(page.locator('#tool-output')).toContainText('Split into 3 scenes', { timeout: 120_000 });
  await expect(page.locator('#tool-output-media')).toBeVisible();
  const links = page.locator('#scene-clip-list a');
  await expect(links).toHaveCount(4);
  await expect(links.nth(0)).toHaveAttribute('download', 'scene-red-green-blue-3s-Scene-001.mp4');
  await expect(links.nth(1)).toHaveAttribute('download', 'scene-red-green-blue-3s-Scene-002.mp4');
  await expect(links.nth(2)).toHaveAttribute('download', 'scene-red-green-blue-3s-Scene-003.mp4');
  await expect(links.nth(3)).toHaveAttribute('download', 'scenes.csv');
}

test('video-scene-split page detects and cuts a three-shot fixture', async ({ page }) => {
  await page.goto('/tools/video-scene-split/');
  await page.fill('#in-threshold', '0.3');
  await page.fill('#in-min_scene', '0.4');
  await page.selectOption('#in-mode', 'reencode');
  await page.fill('#in-crf', '30');
  await page.selectOption('#in-preset', 'ultrafast');
  await page.setInputFiles('#in-file', fixture('scene-red-green-blue-3s.mp4'));

  await waitForSplit(page);
  const src = await page.locator('#tool-output-media').getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4;base64,/);
});

test('video-scene-split deep-link prefills threshold and copy mode', async ({ page }) => {
  await page.goto('/tools/video-scene-split/?threshold=0.3&min_scene=0.4&mode=copy&keep_audio=false');
  await expect(page.locator('#in-threshold')).toHaveValue('0.3');
  await expect(page.locator('#in-min_scene')).toHaveValue('0.4');
  await expect(page.locator('#in-mode')).toHaveValue('copy');
  await expect(page.locator('#in-keep_audio')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture('scene-red-green-blue-3s.mp4'));

  await waitForSplit(page);
});

test('video-scene-split reports no cuts at a high threshold', async ({ page }) => {
  await page.goto('/tools/video-scene-split/');
  await page.fill('#in-threshold', '1');
  await page.fill('#in-min_scene', '0.4');
  await page.setInputFiles('#in-file', fixture('scene-red-green-blue-3s.mp4'));

  await expect(page.locator('#tool-output')).toContainText('No scene changes detected', { timeout: 120_000 });
  await expect(page.locator('#scene-clip-list')).toBeHidden();
});

test('video-scene-split rejects threshold above the cap', async ({ page }) => {
  await page.goto('/tools/video-scene-split/');
  await page.fill('#in-threshold', '1.1');
  await page.setInputFiles('#in-file', fixture('scene-red-green-blue-3s.mp4'));

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 30_000 });
  await expect(page.locator('#tool-output')).toContainText('threshold must be between 0.0 and 1.0');
});
