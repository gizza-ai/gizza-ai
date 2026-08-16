import { test, expect } from './fixtures';

function makeWav(sampleRate = 8000, channels = 1, samples = 80, amplitude = 0.5): Buffer {
  const data = Buffer.alloc(samples * channels * 2);
  for (let i = 0; i < samples * channels; i++) {
    data.writeInt16LE(Math.round(amplitude * 32767), i * 2);
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
  return Buffer.concat([header, data]);
}

const HALF_SCALE_WAV = makeWav();
const HALF_SCALE_B64 = HALF_SCALE_WAV.toString('base64');

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('audio-rms-level-report page emits exact JSON for a half-scale WAV', async ({ page }) => {
  await page.goto('/tools/audio-rms-level-report/');
  await setTextarea(page.locator('#in-input'), HALF_SCALE_B64);
  await page.selectOption('#in-output', 'json');
  await page.fill('#in-rms_window_ms', '50');
  await page.fill('#in-clip_threshold', '0.99');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"sample_rate": 8000', { timeout: 15000 });
  await expect(out).toContainText('"duration_s": 0.01');
  await expect(out).toContainText('"label": "M"');
  await expect(out).toContainText('"rms_dbfs": -6.021');
  await expect(out).toContainText('"peak_dbfs": -6.021');
  await expect(out).toContainText('"average_dbfs": -6.021');
  await expect(out).toContainText('"clipped_samples": 0');
});

test('audio-rms-level-report page supports CSV output', async ({ page }) => {
  await page.goto('/tools/audio-rms-level-report/');
  await setTextarea(page.locator('#in-input'), HALF_SCALE_B64);
  await page.selectOption('#in-output', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('channel,label,samples,rms_dbfs,peak_dbfs,average_dbfs', { timeout: 15000 });
  await expect(out).toContainText('1,M,80,-6.021,-6.021,-6.021');
  await expect(out).toContainText('overall,overall,80,-6.021,-6.021,-6.021');
});

test('audio-rms-level-report deep-link pre-fills hex input and non-default controls', async ({ page }) => {
  const params = new URLSearchParams({
    input: HALF_SCALE_WAV.toString('hex'),
    input_format: 'hex',
    output: 'report',
    rms_window_ms: '10',
    clip_threshold: '0.50',
  });
  await page.goto(`/tools/audio-rms-level-report/?${params.toString()}`);

  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-rms_window_ms')).toHaveValue('10');
  await expect(page.locator('#in-clip_threshold')).toHaveValue('0.50');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Audio level report', { timeout: 15000 });
  await expect(out).toContainText('M               -6.021      -6.021      -6.021');
  await expect(out).toContainText('clipped 80 sample(s) = 100%');
});
