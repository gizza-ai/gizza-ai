import { test, expect } from './fixtures';

const tool = '/tools/svg-security-linter/';
const hostileSvg = '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">\n' +
  '  <script>alert(document.domain)</script>\n' +
  '  <a href="javascript:alert(2)" target="_blank"><text>go</text></a>\n' +
  '  <image href="https://evil.example/pixel.png"/>\n' +
  '</svg>';
const remoteOnlySvg = '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://cdn.example/a.png"/></svg>';
const onloadOnlySvg = '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><rect width="1" height="1"/></svg>';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  svg: string,
  minSeverity = 'all',
  allowExternal = 'false',
  ignore = '',
  format = 'text',
): Promise<string> {
  return await page.evaluate(
    async ({ svg, minSeverity, allowExternal, ignore, format }) => {
      const mod = await import('/tools/svg-security-linter/gizza_ai_svg_security_linter_web.js');
      await mod.default('/tools/svg-security-linter/gizza_ai_svg_security_linter_web_bg.wasm');
      return mod.run(svg, minSeverity, allowExternal, ignore, format);
    },
    { svg, minSeverity, allowExternal, ignore, format },
  );
}

test('svg-security-linter page reports hostile SVG with exact verdict and rule codes', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-svg'), hostileSvg);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('SVG security lint · verdict: unsafe · 5 findings · 3 high · 1 medium · 1 low', {
    timeout: 15_000,
  });
  await expect(out).toContainText('EVENT-HANDLER');
  await expect(out).toContainText('SCRIPT');
  await expect(out).toContainText('JS-URL');
  await expect(out).toContainText('EXTERNAL-REF');
  await expect(out).toContainText('ANCHOR-TARGET');
});

test('svg-security-linter query-param deep link runs high-severity JSON', async ({ page }) => {
  const qs = new URLSearchParams({
    svg: '<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="10" height="10"><iframe src="https://evil.example/"></iframe></foreignObject></svg>',
    min_severity: 'high',
    allow_external: 'false',
    ignore: '',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-svg')).toHaveValue(/<foreignObject/, { timeout: 15_000 });
  await expect(page.locator('#in-min_severity')).toHaveValue('high');
  await expect(page.locator('#in-format')).toHaveValue('json');
  const text = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  const parsed = JSON.parse(text ?? '');
  expect(parsed.verdict).toBe('unsafe');
  expect(parsed.summary.high).toBeGreaterThanOrEqual(2);
  expect(parsed.findings.map((f) => f.code)).toContain('FOREIGN-OBJECT');
});

test('svg-security-linter wasm covers severity, format, checkbox, ignore and cap boundary', async ({ page }) => {
  await page.goto(tool);

  const highOnly = await runWasm(page, hostileSvg, 'high', 'false', '', 'text');
  expect(highOnly).toContain('SVG security lint · verdict: unsafe · 3 findings · 3 high · 0 medium · 0 low');
  expect(highOnly).toContain('2 finding(s) below the selected severity are hidden');
  expect(highOnly).not.toContain('EXTERNAL-REF');

  const jsonText = await runWasm(page, hostileSvg, 'all', 'false', '', 'json');
  const json = JSON.parse(jsonText);
  expect(json.verdict).toBe('unsafe');
  expect(json.summary.medium).toBe(1);
  expect(json.findings[0].code).toBe('EVENT-HANDLER');

  const csv = await runWasm(page, hostileSvg, 'high', 'false', '', 'csv');
  expect(csv.split('\n')[0]).toBe('line,column,severity,code,element,attribute,message,snippet');
  expect(csv).toContain('EVENT-HANDLER,svg,onload');

  const externalBlocked = await runWasm(page, remoteOnlySvg, 'all', 'false', '', 'text');
  expect(externalBlocked).toContain('verdict: review');
  expect(externalBlocked).toContain('EXTERNAL-REF');
  const externalAllowed = await runWasm(page, remoteOnlySvg, 'all', 'true', '', 'text');
  expect(externalAllowed).toContain('verdict: clean');
  expect(externalAllowed).not.toContain('EXTERNAL-REF');

  const ignored = await runWasm(page, onloadOnlySvg, 'all', 'false', 'EVENT-HANDLER', 'text');
  expect(ignored).toContain('verdict: clean');

  const exactlyAtCap = `<svg>${'a'.repeat(999_989)}</svg>`;
  expect(exactlyAtCap.length).toBe(1_000_000);
  const capOk = await runWasm(page, exactlyAtCap, 'all', 'false', '', 'text');
  expect(capOk).toContain('verdict: clean');

  const oneOver = `<svg>${'a'.repeat(999_990)}</svg>`;
  expect(oneOver.length).toBe(1_000_001);
  await expect(runWasm(page, oneOver, 'all', 'false', '', 'text')).rejects.toThrow(/too large/);
});

test('svg-security-linter page renders CSV via the format select', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-svg'), hostileSvg);
  await page.selectOption('#in-format', 'csv');
  await expect(page.locator('#tool-output')).toContainText('line,column,severity,code,element,attribute,message,snippet', {
    timeout: 15_000,
  });
});

test('svg-security-linter page checkbox allows external references', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-svg'), remoteOnlySvg);
  await expect(page.locator('#tool-output')).toContainText('EXTERNAL-REF', { timeout: 15_000 });
  await page.check('#in-allow_external');
  await expect(page.locator('#tool-output')).toContainText('verdict: clean', { timeout: 15_000 });
});
