import { test, expect } from './fixtures';

const tool = '/tools/sphere-to-wav/';

// The demo file behind the page's example chips: a real NIST SPHERE file with a
// 256-byte ASCII header, 8 kHz mono 16-bit BIG-endian PCM, 20 sample frames.
const DEMO =
  'TklTVF8xQQogICAgMjU2CnNhbXBsZV9yYXRlIC1pIDgwMDAKY2hhbm5lbF9jb3VudCAtaSAxCnNhbXBsZV9uX2J5dGVzIC1pIDIKc2FtcGxlX2J5dGVfZm9ybWF0IC1zMiAxMApzYW1wbGVfY29kaW5nIC1zMyBwY20Kc2FtcGxlX2NvdW50IC1pIDIwCmVuZF9oZWFkCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAAAD+Ad4ShYLgsuTCkTHv8RQQF48YTjRtht0kHRf9Y83+rtY/0PDRM=';

// Samples as stored (big-endian) vs. what a WAV must contain (little-endian).
const RAW_LE = '0000e00fe11d58280b2e4c2e1329ff1e4111780184f146e36dd841d27fd13cd6eadf63ed0ffd130d';
const TAIL_10 = '84f146e36dd841d27fd13cd6eadf63ed0ffd130d';

async function setInput(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

// Call the wasm export directly for the advertised-values matrix — same module
// the page driver loads, without driving nine controls per case.
async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    input?: string;
    input_format?: string;
    output?: string;
    encoding?: string;
    channel?: string;
    container?: string;
    byte_order?: string;
    start_sample?: string;
    max_samples?: string;
  } = {},
) {
  const p = {
    input: DEMO,
    input_format: 'auto',
    output: 'hex',
    encoding: 'pcm16',
    channel: 'all',
    container: 'raw',
    byte_order: 'auto',
    start_sample: '0',
    max_samples: '0',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/sphere-to-wav/gizza_ai_sphere_to_wav_web.js');
    await mod.default('/tools/sphere-to-wav/gizza_ai_sphere_to_wav_web_bg.wasm');
    return mod.run(
      args.input,
      args.input_format,
      args.output,
      args.encoding,
      args.channel,
      args.container,
      args.byte_order,
      args.start_sample,
      args.max_samples,
    );
  }, p);
}

test('sphere-to-wav page converts a big-endian SPHERE file to a WAV data URL', async ({ page }) => {
  await page.goto(tool);
  await setInput(page, DEMO);

  // 84-byte WAV: 44-byte RIFF header + 40 byte-swapped sample bytes.
  await expect(page.locator('#tool-output')).toHaveText(
    'data:audio/wav;base64,UklGRkwAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YSgAAAAAAOAP4R1YKAsuTC4TKf8eQRF4AYTxRuNt2EHSf9E81urfY+0P/RMN',
    { timeout: 15_000 },
  );
});

test('sphere-to-wav page byte-swaps samples into raw little-endian PCM', async ({ page }) => {
  await page.goto(tool);
  await setInput(page, DEMO);
  await page.selectOption('#in-output', 'hex');
  await page.selectOption('#in-container', 'raw');

  await expect(page.locator('#tool-output')).toHaveText(RAW_LE, { timeout: 15_000 });
});

test('sphere-to-wav page reports the parsed header fields', async ({ page }) => {
  await page.goto(tool);
  await setInput(page, DEMO);
  await page.selectOption('#in-output', 'info');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('header_bytes     256', { timeout: 15_000 });
  await expect(out).toContainText('sample_byte_format 10 (-s2)');
  await expect(out).toContainText('byte order       big-endian (sample_byte_format 10)');
  await expect(out).toContainText('sample frames    20 (0.0025 s)');
  await expect(out).toContainText('encoding         16-bit signed PCM little-endian (s16le)');
});

test('sphere-to-wav deep-link pre-fills the window and renders the trailing frames', async ({ page }) => {
  const qs = new URLSearchParams({
    input: DEMO,
    input_format: 'auto',
    output: 'hex',
    encoding: 'pcm16',
    channel: 'all',
    container: 'raw',
    byte_order: 'auto',
    start_sample: '10',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('hex', { timeout: 15_000 });
  await expect(page.locator('#in-container')).toHaveValue('raw');
  await expect(page.locator('#in-start_sample')).toHaveValue('10');
  await expect(page.locator('#tool-output')).toHaveText(TAIL_10, { timeout: 15_000 });
});

test('sphere-to-wav wasm covers every advertised value, limit and error', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  // Encodings: source keeps the 16-bit samples, ulaw/alaw compand to 1 byte each.
  await expect(runWasm(page)).resolves.toBe(RAW_LE);
  await expect(runWasm(page, { encoding: 'source' })).resolves.toBe(RAW_LE);
  await expect(runWasm(page, { encoding: 'ulaw' })).resolves.toBe('ffafa19b98989ba0aee031221b18181a1f2c54b4');
  await expect(runWasm(page, { encoding: 'alaw' })).resolves.toBe('d59a88b1b2b2b18b84c21909363332313507729f');

  // Byte-order override reads the same bytes the other way round.
  expect(await runWasm(page, { byte_order: 'little', max_samples: '2' })).toBe('00000fe0');

  // Windowing and channel validation.
  expect(await runWasm(page, { start_sample: '10' })).toBe(TAIL_10);
  expect(await runWasm(page, { max_samples: '2' })).toBe('0000e00f');
  await expect(runWasm(page, { channel: '2' })).rejects.toThrow(/channel=2 needs at least 2 channels/);
  await expect(runWasm(page, { start_sample: '20' })).rejects.toThrow(/start_sample 20 is past the end/);

  // Input forms: hex and data: URI decode to the same audio as base64.
  const asBase64 = await runWasm(page, { output: 'base64', container: 'wav' });
  const asDataUri = await runWasm(page, {
    input: `data:audio/x-nist;base64,${DEMO}`,
    output: 'base64',
    container: 'wav',
  });
  expect(asDataUri).toBe(asBase64);
  const asHex = await runWasm(page, {
    input: Buffer.from(DEMO, 'base64').toString('hex'),
    input_format: 'hex',
    output: 'base64',
    container: 'wav',
  });
  expect(asHex).toBe(asBase64);
  await expect(runWasm(page, { input: 'UklGRiQAAABXQVZFZm10IBAAAAABAAEA' })).rejects.toThrow(
    /not a NIST SPHERE file/,
  );

  // Enum validation names the accepted values.
  await expect(runWasm(page, { output: 'yaml' })).rejects.toThrow(
    /output must be one of data_url, base64, hex, info/,
  );
  await expect(runWasm(page, { input: '' })).rejects.toThrow(/input is empty/);
});

test('sphere-to-wav ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Convert to WAV',
    'Read the header',
    'Raw PCM as hex',
    'Mu-law telephone copy',
    'Last 10 frames only',
  ]);
});
