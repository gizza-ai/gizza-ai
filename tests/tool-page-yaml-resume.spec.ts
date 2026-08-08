import { test, expect } from './fixtures';

const YAML = `basics:
  name: Ada Lovelace
  label: Analytical Engineer
  email: ada@example.com
  summary: Mathematician who wrote the first published algorithm.
work:
  - name: Analytical Engine Co
    position: Lead Engineer
    startDate: "1843-01"
    endDate: "1852-11"
    highlights:
      - Published the first algorithm intended for a machine
skills:
  - name: Mathematics
    level: Expert
    keywords: [Algorithms, Analysis]`;

async function runWasm(
  page,
  data: string,
  format = 'auto',
  theme = 'modern',
  accent = '#2563eb',
  font = 'sans',
  fontSize = '10.5',
  pageSize = 'letter',
  margin = '0.5',
  dateFormat = 'month-year',
  sections = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/yaml-resume/gizza_ai_yaml_resume_web.js');
    await mod.default('/tools/yaml-resume/gizza_ai_yaml_resume_web_bg.wasm');
    return mod.run(args.data, args.format, args.theme, args.accent, args.font, args.fontSize, args.pageSize, args.margin, args.dateFormat, args.sections);
  }, { data, format, theme, accent, font, fontSize, pageSize, margin, dateFormat, sections });
}

test('yaml-resume wasm renders a self-contained modern HTML resume', async ({ page }) => {
  await page.goto('/tools/yaml-resume/');
  const html = await runWasm(page, YAML);

  expect(html).toContain('<!DOCTYPE html>');
  expect(html).toContain('<h1>Ada Lovelace</h1>');
  expect(html).toContain('theme-modern');
  expect(html).toContain('--accent: #2563eb;');
  expect(html).toContain('@page { size: letter; margin: 0.5in; }');
  expect(html).toContain('Jan 1843 – Nov 1852');
  expect(html).toContain('<h2>Experience</h2>');
  expect(html).toContain('Algorithms, Analysis');
});

test('yaml-resume wasm covers enum choices, color forms, and slider boundaries', async ({ page }) => {
  await page.goto('/tools/yaml-resume/');

  for (const theme of ['classic', 'modern', 'compact', 'ats']) {
    await expect(runWasm(page, YAML, 'yaml', theme)).resolves.toContain(`theme-${theme}`);
  }
  for (const dateFormat of ['month-year', 'year', 'iso']) {
    await expect(runWasm(page, YAML, 'yaml', 'modern', '#2563eb', 'sans', '10.5', 'letter', '0.5', dateFormat)).resolves.toContain('<h1>Ada Lovelace</h1>');
  }

  await expect(runWasm(page, YAML, 'yaml', 'modern', '#0f0')).resolves.toContain('--accent: #0f0;');
  await expect(runWasm(page, YAML, 'yaml', 'modern', '#0f766e')).resolves.toContain('--accent: #0f766e;');
  await expect(runWasm(page, YAML, 'yaml', 'modern', 'navy', 'serif', '8', 'a4', '0.25')).resolves.toContain('@page { size: A4; margin: 0.25in; }');
  await expect(runWasm(page, YAML, 'yaml', 'modern', '#2563eb', 'sans', '14', 'letter', '1.5')).resolves.toContain('--fs: 14pt;');
  await expect(runWasm(page, YAML, 'yaml', 'modern', 'red; }')).rejects.toThrow(/invalid accent/);
});

test('yaml-resume page renders raw HTML output and honors section filtering', async ({ page }) => {
  await page.goto('/tools/yaml-resume/');
  await page.fill('#in-data', YAML);
  await page.selectOption('#in-theme', 'ats');
  await page.fill('#in-sections', 'skills');

  await expect(page.locator('#tool-output')).toContainText('<h1>Ada Lovelace</h1>', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('theme-ats');
  await expect(page.locator('#tool-output')).toContainText('<h2>Skills</h2>');
  await expect(page.locator('#tool-output')).not.toContainText('<h2>Experience</h2>');
});

test('yaml-resume deep-link prefills controls and renders compact JSON', async ({ page }) => {
  const json = '{"basics":{"name":"Alan Turing","label":"Mathematician"},"work":[{"name":"NPL","position":"Scientist","startDate":"1945-10"}]}';
  const params = new URLSearchParams({
    data: json,
    format: 'json',
    theme: 'compact',
    accent: '#0f766e',
    font: 'serif',
    font_size: '9.5',
    page_size: 'a4',
    margin: '0.4',
    date_format: 'year',
    sections: 'work',
  });

  await page.goto(`/tools/yaml-resume/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(json, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-theme')).toHaveValue('compact');
  await expect(page.locator('#in-font')).toHaveValue('serif');
  await expect(page.locator('#in-page_size')).toHaveValue('a4');
  await expect(page.locator('#tool-output')).toContainText('<h1>Alan Turing</h1>', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('theme-compact');
  await expect(page.locator('#tool-output')).toContainText('1945 – Present');
});
