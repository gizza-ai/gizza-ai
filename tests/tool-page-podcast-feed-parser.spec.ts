import { test, expect } from './fixtures';

const FEED = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title>Example Podcast</title>
    <description>A sample feed</description>
    <link>https://example.com/podcast</link>
    <itunes:author>Ada</itunes:author>
    <item>
      <title>Episode Two</title>
      <pubDate>Tue, 02 Jan 2024 10:00:00 GMT</pubDate>
      <itunes:duration>01:02:03</itunes:duration>
      <enclosure url="https://cdn.example.com/ep2.mp3" type="audio/mpeg" length="12345" />
      <guid>ep2</guid>
    </item>
    <item>
      <title>Episode One</title>
      <pubDate>Mon, 01 Jan 2024 10:00:00 GMT</pubDate>
      <itunes:duration>15:30</itunes:duration>
      <enclosure url="https://cdn.example.com/ep1.mp3" type="audio/mpeg" />
      <guid>ep1</guid>
    </item>
  </channel>
</rss>`;

test('podcast-feed-parser extracts podcast metadata and episodes', async ({ page }) => {
  await page.goto('/tools/podcast-feed-parser/');
  await page.fill('#in-feed', FEED);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"title": "Example Podcast"', { timeout: 15_000 });
  await expect(out).toContainText('"episode_count": 2');
  await expect(out).toContainText('"audio_url": "https://cdn.example.com/ep2.mp3"');
  await expect(out).toContainText('"duration": "01:02:03"');
});

test('podcast-feed-parser limit and oldest order', async ({ page }) => {
  await page.goto('/tools/podcast-feed-parser/');
  await page.fill('#in-feed', FEED);
  await page.fill('#in-limit', '1');
  await page.fill('#in-order', 'oldest');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"episode_count": 1', { timeout: 15_000 });
  await expect(out).toContainText('"title": "Episode One"');
  await expect(out).not.toContainText('"title": "Episode Two"');
});
