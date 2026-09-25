import { test, expect } from './fixtures';

const participants = 'Alice@Europe/London, Bob@America/New_York';

test('meeting-time-finder ranks a London/New York overlap on the page', async ({ page }) => {
  await page.goto('/tools/meeting-time-finder/');
  await page.fill('#in-participants', participants);
  await page.fill('#in-date', '2026-10-01');
  await page.fill('#in-duration_minutes', '60');
  await page.selectOption('#in-granularity_minutes', '30');
  await page.fill('#in-work_start', '09:00');
  await page.fill('#in-work_end', '17:00');
  await page.fill('#in-max_results', '3');
  await page.selectOption('#in-clock', '24h');
  await page.fill('#in-display_zone', 'Europe/London');
  await page.check('#in-skip_weekends');
  await page.check('#in-allow_partial');
  await page.selectOption('#in-output_format', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('works for all 2', { timeout: 15000 });
  await expect(out).toContainText('slots shown in Europe/London');
  await expect(out).toContainText('Alice');
  await expect(out).toContainText('Bob');
});

test('meeting-time-finder supports timeline output and 12-hour clock', async ({ page }) => {
  await page.goto('/tools/meeting-time-finder/');
  await page.fill('#in-participants', 'Ava@America/Los_Angeles, Chen@Asia/Tokyo:10-18');
  await page.fill('#in-date', '2026-10-01');
  await page.fill('#in-duration_minutes', '45');
  await page.selectOption('#in-granularity_minutes', '15');
  await page.fill('#in-display_zone', 'America/Los_Angeles');
  await page.selectOption('#in-clock', '12h');
  await page.selectOption('#in-output_format', 'timeline');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Hour-by-hour overlap', { timeout: 15000 });
  await expect(out).toContainText('Ava');
  await expect(out).toContainText('Chen');
  await expect(out).toContainText('EVERYONE');
});

test('meeting-time-finder deep-link prefills and computes table output', async ({ page }) => {
  const params = new URLSearchParams({
    participants,
    date: '2026-10-01',
    duration_minutes: '30',
    granularity_minutes: '30',
    work_start: '09:00',
    work_end: '17:00',
    max_results: '2',
    clock: '24h',
    display_zone: 'Europe/London',
    skip_weekends: 'true',
    allow_partial: 'true',
    output_format: 'table',
  });
  await page.goto(`/tools/meeting-time-finder/?${params.toString()}`);

  await expect(page.locator('#in-participants')).toHaveValue(participants, { timeout: 15000 });
  await expect(page.locator('#in-date')).toHaveValue('2026-10-01');
  await expect(page.locator('#in-output_format')).toHaveValue('table');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('#  Start', { timeout: 15000 });
  await expect(out).toContainText('Start');
  await expect(out).toContainText('All');
});

test('meeting-time-finder reports an unknown timezone error', async ({ page }) => {
  await page.goto('/tools/meeting-time-finder/');
  await page.fill('#in-participants', 'Mars/Olympus');
  await page.fill('#in-date', '2026-10-01');

  await expect(page.locator('#tool-output')).toContainText('unknown timezone', { timeout: 15000 });
});
