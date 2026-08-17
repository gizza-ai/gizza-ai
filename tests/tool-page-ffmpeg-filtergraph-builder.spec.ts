import { test, expect } from './fixtures';

const defaultSteps = 'scale to 720p\ncrop to square\nfade in 1s';
const defaultGraph = "[0:v]scale=-2:720,crop='min(iw,ih)':'min(iw,ih)',fade=t=in:st=0:d=1[out]";

test('ffmpeg-filtergraph-builder page emits the default filter_complex exactly', async ({ page }) => {
  await page.goto('/tools/ffmpeg-filtergraph-builder/');
  await page.fill('#in-steps', defaultSteps);
  await page.selectOption('#in-stream', 'video');
  await page.selectOption('#in-output', 'filter_complex');
  await page.fill('#in-input_label', 'auto');
  await page.fill('#in-output_label', 'out');
  await page.fill('#in-input_file', 'input.mp4');
  await page.fill('#in-output_file', 'output.mp4');
  await page.uncheck('#in-explain');
  await expect(page.locator('#tool-output')).toHaveText(defaultGraph, { timeout: 15_000 });
});

test('ffmpeg-filtergraph-builder deep link renders a full command', async ({ page }) => {
  const params = new URLSearchParams({
    steps: 'trim 5 to 20\nspeed 2x',
    stream: 'video',
    output: 'command',
    input_label: 'auto',
    output_label: 'vout',
    input_file: 'clip.mov',
    output_file: 'fast.mp4',
    explain: 'false',
  });
  await page.goto(`/tools/ffmpeg-filtergraph-builder/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('command', { timeout: 15_000 });
  await expect(page.locator('#in-output_label')).toHaveValue('vout');
  await expect(page.locator('#tool-output')).toHaveText(
    'ffmpeg -i clip.mov -filter_complex "[0:v]trim=start=5:end=20,setpts=PTS-STARTPTS,setpts=0.5*PTS[vout]" -map "[vout]" -map "0:a?" fast.mp4',
    { timeout: 15_000 },
  );
});

test('ffmpeg-filtergraph-builder page handles audio plus explain checkbox', async ({ page }) => {
  await page.goto('/tools/ffmpeg-filtergraph-builder/');
  await page.fill('#in-steps', 'normalize\nfade in 2\nspeed 4x');
  await page.selectOption('#in-stream', 'audio');
  await page.selectOption('#in-output', 'filter_chain');
  await page.check('#in-explain');
  await expect(page.locator('#tool-output')).toHaveText(
    'loudnorm=I=-16:TP=-1.5:LRA=11,afade=t=in:st=0:d=2,atempo=2,atempo=2\n\n# How each step compiled:\n# 1. normalize → loudnorm=I=-16:TP=-1.5:LRA=11\n# 2. fade in 2 → afade=t=in:st=0:d=2\n# 3. speed 4x → atempo=2,atempo=2',
    { timeout: 15_000 },
  );
});
