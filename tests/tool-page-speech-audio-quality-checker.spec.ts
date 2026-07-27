import { test, expect } from './fixtures';

function makeWavBase64(sampleRate = 16000, channels = 1, samples = 800): string {
  const data = Buffer.alloc(samples * channels * 2);
  for (let i = 0; i < samples * channels; i++) {
    const frame = Math.floor(i / channels);
    const amp = frame < samples * 0.6 ? 0.5 : 0.001;
    const s = Math.round(Math.sin(frame * 0.3) * amp * 32767);
    data.writeInt16LE(s, i * 2);
  }
  const header = Buffer.alloc(44);
  header.write('RIFF', 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write('WAVE', 8);
  header.write('fmt ', 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
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

test('speech-audio-quality-checker reports a clean 16 kHz mono WAV as ready', async ({ page }) => {
  await page.goto('/tools/speech-audio-quality-checker/');
  await page.fill('#in-input', GOOD_WAV);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Speech audio quality — ASR readiness', { timeout: 15000 });
  await expect(out).toContainText('[PASS] Sample rate');
  await expect(out).toContainText('[PASS] Channels');
  await expect(out).toContainText('Verdict: READY for ASR / transcription');
});

test('speech-audio-quality-checker emits machine-readable JSON', async ({ page }) => {
  await page.goto('/tools/speech-audio-quality-checker/');
  await page.fill('#in-input', GOOD_WAV);
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"sample_rate":16000', { timeout: 15000 });
  await expect(out).toContainText('"channels":1');
  await expect(out).toContainText('"verdict":"ready"');
});

test('speech-audio-quality-checker deep-link honors thresholds and warns on stereo', async ({ page }) => {
  const params = new URLSearchParams({
    input: makeWavBase64(16000, 2),
    input_format: 'base64',
    output: 'report',
    target_sample_rate: '16000',
    min_snr_db: '20',
    max_clipping_pct: '1.0',
    clipping_threshold: '0.99',
  });

  await page.goto(`/tools/speech-audio-quality-checker/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-target_sample_rate')).toHaveValue('16000');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('[WARN] Channels', { timeout: 15000 });
  await expect(out).toContainText('Verdict: USABLE with caveats');
});
