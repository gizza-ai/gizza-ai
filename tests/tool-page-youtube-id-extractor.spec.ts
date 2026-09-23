import { test, expect } from './fixtures';

const tool = '/tools/youtube-id-extractor/';
const sample = 'https://youtu.be/dQw4w9WgXcQ?t=90';

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    urls: sample,
    format: 'text',
    timestamp: 'both',
    thumbnail: 'hqdefault',
    canonical: 'true',
    embed: 'false',
    strict: 'false',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/youtube-id-extractor/gizza_ai_youtube_id_extractor_web.js');
    await mod.default('/tools/youtube-id-extractor/gizza_ai_youtube_id_extractor_web_bg.wasm');
    return mod.run(
      args.urls,
      args.format,
      args.timestamp,
      args.thumbnail,
      args.canonical,
      args.embed,
      args.strict,
    );
  }, p);
}

test('youtube-id-extractor page renders a video ID report', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-urls', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('id: dQw4w9WgXcQ', { timeout: 20_000 });
  await expect(output).toContainText('start: 90s (1:30)');
  await expect(output).toContainText('canonical: https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90s');
});

test('youtube-id-extractor deep link can prefill CSV embed output', async ({ page }) => {
  const qs = new URLSearchParams({
    urls: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90',
    format: 'csv',
    timestamp: 'seconds',
    thumbnail: 'none',
    canonical: 'false',
    embed: 'true',
    strict: 'false',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-urls')).toHaveValue('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-timestamp')).toHaveValue('seconds');
  await expect(page.locator('#in-thumbnail')).toHaveValue('none');
  await expect(page.locator('#in-embed')).toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('input,kind,id,start_seconds,playlist,index,embed,error', {
    timeout: 20_000,
  });
  await expect(output).toContainText('dQw4w9WgXcQ,90,,,https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?start=90');
});

test('youtube-id-extractor wasm covers hosts, formats, thumbnails, and strict errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-urls');

  expect(await runWasm(page, { urls: 'https://piped.video/watch?v=dQw4w9WgXcQ&t=1m30s' })).toContain(
    'start: 90s (1:30)',
  );
  expect(await runWasm(page, { urls: 'https://www.youtube.com/shorts/dQw4w9WgXcQ', thumbnail: 'maxresdefault' })).toContain(
    'https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg',
  );
  const json = JSON.parse(await runWasm(page, { format: 'json', embed: 'true' }));
  expect(json[0].id).toBe('dQw4w9WgXcQ');
  expect(json[0].start_seconds).toBe(90);
  expect(json[0].embed).toContain('youtube-nocookie.com/embed/dQw4w9WgXcQ');

  const csv = await runWasm(page, {
    urls: 'https://yewtu.be/watch?v=dQw4w9WgXcQ&list=PL1234567890123&index=2',
    format: 'csv',
    thumbnail: 'none',
    canonical: 'false',
    embed: 'false',
  });
  expect(csv).toContain('input,kind,id,start_seconds,start_clock,playlist,index,error');
  expect(csv).toContain('dQw4w9WgXcQ,,,PL1234567890123,2');

  await expect(runWasm(page, { urls: 'not a youtube link', strict: 'true' })).rejects.toThrow(
    /strict mode: could not resolve/,
  );
});
