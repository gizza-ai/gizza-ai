import { test, expect } from './fixtures';

function makeWavBase64(sampleRate = 8000, channels = 1, samples = 8000, amplitude = 0.5): string {
  const data = Buffer.alloc(samples * channels * 2);
  for (let i = 0; i < samples * channels; i++) {
    const sample = Math.round(amplitude * 32767);
    data.writeInt16LE(sample, i * 2);
  }
  const header = Buffer.alloc(44);
  header.write('RIFF', 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write('WAVE', 8);
  header.write('fmt ', 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(channels, 22);
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(sampleRate * channels * 2, 28);
  header.writeUInt16LE(channels * 2, 32);
  header.writeUInt16LE(16, 34);
  header.write('data', 36);
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]).toString('base64');
}

const GOOD_WAV = makeWavBase64();

async function fillLarge(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('video-audio-rms-timeline page emits exact CSV rows for a constant WAV', async ({ page }) => {
  await page.goto('/tools/video-audio-rms-timeline/');
  await fillLarge(page.locator('#in-input'), GOOD_WAV);
  await page.fill('#in-window_ms', '500');
  await page.fill('#in-hop_ms', '0');
  await page.selectOption('#in-unit', 'linear');
  await page.selectOption('#in-output', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('window,start_s,end_s,rms,peak', { timeout: 15000 });
  await expect(out).toContainText('0,0,0.5,0.5,0.5');
  await expect(out).toContainText('1,0.5,1,0.5,0.5');
});

test('video-audio-rms-timeline page supports JSON output and overlapping hops', async ({ page }) => {
  await page.goto('/tools/video-audio-rms-timeline/');
  await fillLarge(page.locator('#in-input'), GOOD_WAV);
  await page.fill('#in-window_ms', '250');
  await page.fill('#in-hop_ms', '125');
  await page.selectOption('#in-unit', 'linear');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"sample_rate": 8000', { timeout: 15000 });
  await expect(out).toContainText('"unit": "linear"');
  await expect(out).toContainText('"window_count": 8');
});

test('video-audio-rms-timeline deep-link pre-fills secondary input format controls', async ({ page }) => {
  const wav = Buffer.from(GOOD_WAV, 'base64');
  const params = new URLSearchParams({
    input: wav.toString('hex'),
    input_format: 'hex',
    window_ms: '1000',
    hop_ms: '0',
    unit: 'dbfs',
    output: 'csv',
  });
  await page.goto(`/tools/video-audio-rms-timeline/?${params.toString()}`);

  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-window_ms')).toHaveValue('1000');
  await expect(page.locator('#in-unit')).toHaveValue('dbfs');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('window,start_s,end_s,rms_dbfs,peak_dbfs', { timeout: 15000 });
  await expect(out).toContainText('0,0,1,-6.021,-6.021');
});
