import { test, expect } from './fixtures';

const SAMPLE = '[{"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-01T18:10:00Z","products":["YouTube"]},' +
  '{"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-02T19:00:00Z","products":["YouTube"]},' +
  '{"header":"YouTube","title":"Watched Rust in 100 Seconds","titleUrl":"https://www.youtube.com/watch?v=5C_HPTJg5ek","subtitles":[{"name":"Fireship"}],"time":"2024-01-03T09:05:00Z","products":["YouTube"]},' +
  '{"header":"YouTube Music","title":"Watched Some Song","titleUrl":"https://music.youtube.com/watch?v=bbbbbbbbbbb","subtitles":[{"name":"Some Artist"}],"time":"2024-01-04T22:00:00Z","products":["YouTube Music"]}]';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('youtube-takeout-stats page emits exact overview stats', async ({ page }) => {
  await page.goto('/tools/youtube-takeout-stats/');
  await setTextarea(page.locator('#in-input'), SAMPLE);
  await page.selectOption('#in-output', 'text');
  await page.selectOption('#in-report', 'overview');
  await page.fill('#in-top', '10');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('YouTube watch history — 3 videos watched', { timeout: 15000 });
  await expect(out).toContainText('Range: 2024-01-01 → 2024-01-03 (3 days, 3 active)');
  await expect(out).toContainText('Unique videos: 2 · Unique channels: 2');
  await expect(out).toContainText('Skipped: 1 YouTube Music');
  await expect(out).toContainText('Rick Astley');
  await expect(out).toContainText('Never Gonna Give You Up');
});

test('youtube-takeout-stats page supports CSV report and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/youtube-takeout-stats/');
  await setTextarea(page.locator('#in-input'), SAMPLE);
  await page.selectOption('#in-output', 'csv');
  await page.selectOption('#in-report', 'channels');
  await page.fill('#in-top', '100');
  await page.check('#in-include_music');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('rank,channel,videos,share_percent', { timeout: 15000 });
  await expect(out).toContainText('1,Rick Astley,2,50.0');
  await expect(out).toContainText('2,Fireship,1,25.0');
  await expect(out).toContainText('3,Some Artist,1,25.0');
});

test('youtube-takeout-stats deep-link pre-fills filters and returns JSON months', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE,
    output: 'json',
    report: 'months',
    top: '1',
    utc_offset: '-5',
    include_ads: 'false',
    include_music: 'false',
    start_date: '2024-01-02',
    end_date: '2024-01-31',
  });
  await page.goto(`/tools/youtube-takeout-stats/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-report')).toHaveValue('months');
  await expect(page.locator('#in-top')).toHaveValue('1');
  await expect(page.locator('#in-utc_offset')).toHaveValue('-5');
  await expect(page.locator('#in-include_music')).not.toBeChecked();
  await expect(page.locator('#in-start_date')).toHaveValue('2024-01-02');
  await expect(page.locator('#in-end_date')).toHaveValue('2024-01-31');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"months"', { timeout: 15000 });
  await expect(out).toContainText('"month": "2024-01"');
  await expect(out).toContainText('"videos": 2');
});
