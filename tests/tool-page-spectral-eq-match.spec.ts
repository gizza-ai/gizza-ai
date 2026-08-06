import { test, expect } from './fixtures';

const TRACK = '63 -12\n250 -8\n1k -6\n4k -10\n12k -18';
const REFERENCE = '63 -10\n250 -8\n1k -6\n4k -8\n12k -14';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('matches a pasted reference and reports bands, loudness offset and ffmpeg chain', async ({ page }) => {
  await page.goto('/tools/spectral-eq-match/');
  await page.fill('#in-track', TRACK);
  await page.fill('#in-reference', REFERENCE);
  await page.selectOption('#in-target_curve', 'reference');
  await page.fill('#in-amount', '100');
  await page.fill('#in-smoothing', '1');
  await page.fill('#in-track_lufs', '-9.5');
  await page.fill('#in-target_lufs', '-14');
  await page.selectOption('#in-output', 'report');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Spectral EQ match: 5 bands', { timeout: 15000 });

  const text = await output(page);
  const flat = text.replace(/[ \t]+/g, ' ');
  expect(flat).toContain('12000 -18.00 -14.00 +2.40 +1.40 +1.40');
  expect(flat).toContain('63 -12.00 -10.00 +0.40 -0.60 -0.60');
  expect(text).toContain('Loudness: -9.5 LUFS -> -14.0 LUFS = -4.50 dB offset');
  expect(text).toContain('equalizer=f=12000:t=q:w=1:g=1.40');
  expect(text).toContain('volume=-4.50dB');
});

test('deep-links a half-strength ffmpeg chain via query params', async ({ page }) => {
  await page.goto(
    '/tools/spectral-eq-match/?track=' +
      encodeURIComponent(TRACK) +
      '&reference=' +
      encodeURIComponent(REFERENCE) +
      '&target_curve=reference&amount=50&smoothing=0&q=1.4&output=ffmpeg',
  );

  const out = page.locator('#tool-output');
  await expect(out).toContainText('ffmpeg -i input.wav -af', { timeout: 15000 });

  const text = await output(page);
  expect(text).toContain(
    'equalizer=f=63:t=q:w=1.4:g=0.20,equalizer=f=250:t=q:w=1.4:g=-0.80,' +
      'equalizer=f=1000:t=q:w=1.4:g=-0.80,equalizer=f=4000:t=q:w=1.4:g=0.20,' +
      'equalizer=f=12000:t=q:w=1.4:g=1.20',
  );
  expect(text).not.toContain('volume=');
  expect(text).not.toContain('Spectral EQ match:');
});
