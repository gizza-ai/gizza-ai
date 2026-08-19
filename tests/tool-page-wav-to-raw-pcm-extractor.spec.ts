import { test, expect } from './fixtures';

const MONO16 = 'UklGRi4AAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQYAAAAAQADgAAA=';
const STEREO16 = 'UklGRjAAAABXQVZFZm10IBAAAAABAAIAgD4AAAD6AAAEABAAZGF0YQgAAAAAQADAAAD/fw==';
const MONO16_HEX = '524946462e00000057415645666d74201000000001000100803e0000007d0000020010006461746106000000004000e00000';

async function setInput(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('wav-to-raw-pcm-extractor page strips a WAV header and returns exact base64 PCM', async ({ page }) => {
  await page.goto('/tools/wav-to-raw-pcm-extractor/');
  await setInput(page, MONO16);

  await expect(page.locator('#tool-output')).toHaveText('AEAA4AAA', { timeout: 15_000 });
});

test('wav-to-raw-pcm-extractor page supports hex input and hex output with line wrapping', async ({ page }) => {
  await page.goto('/tools/wav-to-raw-pcm-extractor/');
  await setInput(page, MONO16_HEX);
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-output', 'hex');
  await page.locator('#in-line_bytes').fill('4');

  await expect(page.locator('#tool-output')).toHaveText('00 40 00 e0\n00 00', { timeout: 15_000 });
});

test('wav-to-raw-pcm-extractor page emits the format report and re-import commands', async ({ page }) => {
  await page.goto('/tools/wav-to-raw-pcm-extractor/');
  await setInput(page, MONO16);
  await page.selectOption('#in-output', 'info');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('sample rate     16000 Hz', { timeout: 15_000 });
  await expect(out).toContainText('encoding        s16le (signed 16-bit little-endian), verbatim payload');
  await expect(out).toContainText('ffmpeg -f s16le -ar 16000 -ac 1 -i out.pcm out.wav');
});

test('wav-to-raw-pcm-extractor page converts sample format and splits stereo channels', async ({ page }) => {
  await page.goto('/tools/wav-to-raw-pcm-extractor/');
  await setInput(page, STEREO16);
  await page.selectOption('#in-output', 'hex');
  await page.selectOption('#in-sample_format', 'source');
  await page.selectOption('#in-channels', 'left');
  await page.locator('#in-line_bytes').fill('0');

  await expect(page.locator('#tool-output')).toHaveText('00400000', { timeout: 15_000 });

  await setInput(page, MONO16);
  await page.selectOption('#in-sample_format', 'u8');
  await page.selectOption('#in-channels', 'all');
  await expect(page.locator('#tool-output')).toHaveText('c06080', { timeout: 15_000 });
});

test('wav-to-raw-pcm-extractor page renders C arrays and honors the line_bytes cap boundary', async ({ page }) => {
  await page.goto('/tools/wav-to-raw-pcm-extractor/');
  await setInput(page, MONO16);
  await page.selectOption('#in-output', 'c_array');
  await page.locator('#in-line_bytes').fill('64');

  await expect(page.locator('#tool-output')).toHaveText(
    '/* raw PCM: s16le, 1 channel, 16000 Hz */\n' +
      'const unsigned char pcm_data[] = {\n' +
      '  0x00, 0x40, 0x00, 0xe0, 0x00, 0x00\n' +
      '};\n' +
      'const unsigned int pcm_data_len = 6;\n',
    { timeout: 15_000 },
  );
});

test('wav-to-raw-pcm-extractor page honors query-param deep links with real output', async ({ page }) => {
  const params = new URLSearchParams({
    input: MONO16,
    input_format: 'base64',
    output: 'hex',
    sample_format: 'source',
    channels: 'all',
    start_frame: '1',
    max_frames: '1',
    line_bytes: '0',
  });

  await page.goto(`/tools/wav-to-raw-pcm-extractor/?${params.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(MONO16);
  await expect(page.locator('#in-output')).toHaveValue('hex');
  await expect(page.locator('#in-start_frame')).toHaveValue('1');
  await expect(page.locator('#in-max_frames')).toHaveValue('1');
  await expect(page.locator('#tool-output')).toHaveText('00e0', { timeout: 15_000 });
});
