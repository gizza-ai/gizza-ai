import { test, expect } from './fixtures';

const srt = `1
00:00:01,000 --> 00:00:04,000
<i>Hello there.</i>

2
00:00:05,500 --> 00:00:07,250
[applause] JOHN: Welcome back.
`;

test('srt-to-plaintext page strips cue numbers, timestamps, tags, effects and speaker labels', async ({ page }) => {
  await page.goto('/tools/srt-to-plaintext/');
  await page.fill('#in-input', srt);
  await page.check('#in-remove_sound_effects');
  await page.check('#in-remove_speaker_labels');

  await expect(page.locator('#tool-output')).toHaveText('Hello there.\nWelcome back.', {
    timeout: 15000,
  });
});

test('srt-to-plaintext honours deep link layout and dedupe settings', async ({ page }) => {
  const rolling = `1
00:00:01,000 --> 00:00:02,000
Hello world

2
00:00:02,000 --> 00:00:03,000
hello world

3
00:00:03,000 --> 00:00:04,000
Next line
`;
  const qs =
    '?input=' + encodeURIComponent(rolling) +
    '&layout=paragraph' +
    '&strip_tags=true' +
    '&dedupe=true';
  await page.goto('/tools/srt-to-plaintext/' + qs);

  await expect(page.locator('#in-layout')).toHaveValue('paragraph', { timeout: 15000 });
  await expect(page.locator('#in-dedupe')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('Hello world Next line', {
    timeout: 15000,
  });
});
