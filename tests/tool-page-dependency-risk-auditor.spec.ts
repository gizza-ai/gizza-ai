import { test, expect } from './fixtures';

const TINY = '{"name":"tiny","engines":{"node":">=20"},"dependencies":{"axios":"*"}}';

const MESSY = `{
  "name": "demo-app",
  "version": "1.0.0",
  "scripts": { "postinstall": "node scripts/setup.js" },
  "dependencies": {
    "axios": "*",
    "chalk": "^4.1.2",
    "internal-tool": "git+ssh://git@github.com/acme/internal-tool.git#main"
  },
  "devDependencies": { "chalk": "^5.3.0" }
}`;

const NPM_LOCK = `{
  "name": "demo-app",
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "demo-app", "version": "1.0.0" },
    "node_modules/chalk": {
      "version": "4.1.2",
      "resolved": "https://registry.npmjs.org/chalk/-/chalk-4.1.2.tgz",
      "integrity": "sha512-oKnbhFyRIXpUuez8iBMmyEa4nbj4IOQyuhc"
    },
    "node_modules/sharp": {
      "version": "0.32.6",
      "resolved": "https://registry.npmjs.org/sharp/-/sharp-0.32.6.tgz",
      "integrity": "sha512-KyLTWwgcR9Oe4d9HwCwNM2l7",
      "hasInstallScript": true
    },
    "node_modules/mirror-pkg": {
      "version": "2.0.0",
      "resolved": "http://npm.internal.example.com/mirror-pkg/-/mirror-pkg-2.0.0.tgz"
    }
  }
}`;

const YARN_LOCK = `# yarn lockfile v1


chalk@^4.1.2:
  version "4.1.2"
  resolved "https://registry.yarnpkg.com/chalk/-/chalk-4.1.2.tgz#hash"
  integrity sha512-abc==

sketchy@^1.0.0:
  version "1.0.0"
  resolved "http://mirror.example.com/sketchy/-/sketchy-1.0.0.tgz"
`;

const PNPM_LOCK = `lockfileVersion: '6.0'

packages:

  /chalk@4.1.2:
    resolution: {integrity: sha512-abc==}
    dev: false

  /esbuild@0.19.0:
    resolution: {integrity: sha512-def==, tarball: http://mirror.example.com/esbuild.tgz}
    requiresBuild: true
    dev: true
`;

/** The complete default-strictness text report for TINY, byte for byte. */
const TINY_TEXT =
  'DEPENDENCY RISK AUDIT — FAIL\n' +
  'Input: package.json\n' +
  'Entries scanned: 1 | Strictness: standard\n' +
  'Risk score: 80/100 (grade B)\n' +
  'Findings: 1 high, 0 medium, 0 low, 0 info\n' +
  '\n' +
  'HIGH\n' +
  '  [wildcard-version] axios (dependencies)\n' +
  '      value: *\n' +
  '      Spec * accepts any published version, so the next install can pull a brand-new ' +
  'release — including a compromised one. Pin a range such as ^1.2.3, or an exact version.\n';

/**
 * Call the page's own wasm export directly. Argument order matches
 * page/meta.toml and web/src/lib.rs.
 */
async function runWasm(
  page,
  manifest: string,
  lockfile = '',
  manifest_format = 'auto',
  strictness = 'standard',
  include_dev = 'true',
  ignore = '',
  fail_on = 'high',
  output = 'text',
): Promise<string> {
  return await page.evaluate(async (args) => {
    const mod = await import(
      '/tools/dependency-risk-auditor/gizza_ai_dependency_risk_auditor_web.js'
    );
    await mod.default(
      '/tools/dependency-risk-auditor/gizza_ai_dependency_risk_auditor_web_bg.wasm',
    );
    return mod.run(
      args.manifest,
      args.lockfile,
      args.manifest_format,
      args.strictness,
      args.include_dev,
      args.ignore,
      args.fail_on,
      args.output,
    );
  }, { manifest, lockfile, manifest_format, strictness, include_dev, ignore, fail_on, output });
}

const rulesOf = (json: string): string[] =>
  JSON.parse(json).findings.map((f: { rule: string }) => f.rule);

test('dependency-risk-auditor wasm returns the exact default report', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  await expect(runWasm(page, TINY)).resolves.toBe(TINY_TEXT);
});

test('dependency-risk-auditor wasm covers every advertised input format', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  // auto-detection and the explicit format enum agree on all four shapes.
  for (const [fixture, format, scanned] of [
    [MESSY, 'package-json', 4],
    [NPM_LOCK, 'package-lock', 3],
    [YARN_LOCK, 'yarn-lock', 2],
    [PNPM_LOCK, 'pnpm-lock', 2],
  ] as [string, string, number][]) {
    const auto = JSON.parse(await runWasm(page, fixture, '', 'auto', 'standard', 'true', '', 'high', 'json'));
    expect(auto.detected_format).toBe(format);
    expect(auto.entries_scanned).toBe(scanned);

    const explicit = JSON.parse(
      await runWasm(page, fixture, '', format, 'standard', 'true', '', 'high', 'json'),
    );
    expect(explicit.detected_format).toBe(format);
    expect(explicit.findings).toEqual(auto.findings);
  }

  // Lockfile-only rules really fire, per source format.
  expect(rulesOf(await runWasm(page, NPM_LOCK, '', 'auto', 'standard', 'true', '', 'high', 'json')))
    .toEqual(expect.arrayContaining([
      'has-install-script', 'missing-integrity', 'insecure-resolved-url', 'third-party-registry',
    ]));
  expect(rulesOf(await runWasm(page, YARN_LOCK, '', 'auto', 'standard', 'true', '', 'high', 'json')))
    .toEqual(expect.arrayContaining(['insecure-resolved-url', 'missing-integrity']));
  expect(rulesOf(await runWasm(page, PNPM_LOCK, '', 'auto', 'standard', 'true', '', 'high', 'json')))
    .toEqual(expect.arrayContaining(['has-install-script', 'insecure-resolved-url']));
});

test('dependency-risk-auditor wasm covers strictness, include_dev, ignore and fail_on', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  // strictness: every enum choice, and the severity floor each one implies.
  const lenient = rulesOf(await runWasm(page, MESSY, '', 'auto', 'lenient', 'true', '', 'high', 'json'));
  expect(lenient).toContain('wildcard-version');
  expect(lenient).not.toContain('duplicate-dependency');

  const standard = rulesOf(await runWasm(page, MESSY, '', 'auto', 'standard', 'true', '', 'high', 'json'));
  expect(standard).toEqual(expect.arrayContaining([
    'git-dependency', 'install-script', 'wildcard-version', 'duplicate-dependency',
  ]));
  expect(standard).not.toContain('range-prefix');

  const strict = rulesOf(await runWasm(page, MESSY, '', 'auto', 'strict', 'true', '', 'high', 'json'));
  expect(strict).toEqual(expect.arrayContaining([
    'range-prefix', 'missing-engines', 'no-lockfile-supplied',
  ]));

  // include_dev: the NON-default (unchecked) state changes the result.
  const devSrc = '{"engines":{"node":">=20"},"dependencies":{"a":"1.0.0"},"devDependencies":{"b":"*"}}';
  expect(rulesOf(await runWasm(page, devSrc, '', 'auto', 'standard', 'true', '', 'high', 'json')))
    .toContain('wildcard-version');
  expect(rulesOf(await runWasm(page, devSrc, '', 'auto', 'standard', 'false', '', 'high', 'json')))
    .not.toContain('wildcard-version');

  // ignore: suppressed rules disappear AND stop counting toward the verdict.
  const ignored = JSON.parse(await runWasm(
    page, MESSY, '', 'auto', 'standard', 'true',
    'wildcard-version, git-dependency, install-script', 'high', 'json',
  ));
  expect(ignored.verdict).toBe('pass');
  expect(ignored.summary.high).toBe(0);
  expect(rulesOf(JSON.stringify(ignored))).not.toContain('wildcard-version');

  // fail_on: every enum choice on a manifest whose worst finding is medium.
  const mediumOnly = '{"engines":{"node":">=20"},"dependencies":{"x":"1.2.3-rc.1"}}';
  const verdict = async (failOn: string) =>
    JSON.parse(await runWasm(page, mediumOnly, '', 'auto', 'standard', 'true', '', failOn, 'json')).verdict;
  expect(await verdict('high')).toBe('pass');
  expect(await verdict('medium')).toBe('fail');
  expect(await verdict('low')).toBe('fail');
  expect(await verdict('info')).toBe('fail');
  expect(await verdict('never')).toBe('pass');
});

test('dependency-risk-auditor wasm renders text, markdown and json output', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  const text = await runWasm(page, TINY, '', 'auto', 'standard', 'true', '', 'high', 'text');
  expect(text).toBe(TINY_TEXT);

  const markdown = await runWasm(page, TINY, '', 'auto', 'standard', 'true', '', 'high', 'markdown');
  expect(markdown).toContain('## Dependency risk audit — FAIL');
  expect(markdown).toContain('| Severity | Rule | Subject | Value | Detail |');
  expect(markdown).toContain('| high | `wildcard-version` | axios (dependencies) | `*` |');

  const json = JSON.parse(await runWasm(page, TINY, '', 'auto', 'standard', 'true', '', 'high', 'json'));
  expect(json).toMatchObject({
    verdict: 'fail',
    detected_format: 'package-json',
    lockfile_format: null,
    strictness: 'standard',
    entries_scanned: 1,
    score: 80,
    grade: 'B',
    truncated: false,
    summary: { high: 1, medium: 0, low: 0, info: 0, total: 1 },
  });
  expect(json.findings[0]).toMatchObject({
    rule: 'wildcard-version',
    severity: 'high',
    subject: 'axios',
    location: 'dependencies',
    value: '*',
  });
});

test('dependency-risk-auditor wasm cross-checks a manifest against its lockfile', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  const pkg = '{"engines":{"node":">=20"},"dependencies":{"chalk":"4.1.1","missing-dep":"1.0.0"}}';
  const json = JSON.parse(
    await runWasm(page, pkg, NPM_LOCK, 'auto', 'standard', 'true', '', 'high', 'json'),
  );
  expect(json.lockfile_format).toBe('package-lock');
  const rules = json.findings.map((f: { rule: string }) => f.rule);
  expect(rules).toEqual(expect.arrayContaining(['unlocked-dependency', 'pin-mismatch']));
  expect(rules).not.toContain('no-lockfile-supplied');
});

test('dependency-risk-auditor wasm rejects malformed input and enforces the size cap', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  await expect(runWasm(page, '   ')).rejects.toThrow(/manifest is empty/);
  await expect(runWasm(page, 'hello world')).rejects.toThrow(/could not detect the input format/);
  await expect(runWasm(page, '{ "dependencies": ')).rejects.toThrow(/did not parse/);
  await expect(runWasm(page, TINY, '', 'toml')).rejects.toThrow(/unknown manifest_format/);
  await expect(runWasm(page, TINY, '', 'auto', 'paranoid')).rejects.toThrow(/unknown strictness/);
  await expect(runWasm(page, TINY, '', 'auto', 'standard', 'true', '', 'sometimes'))
    .rejects.toThrow(/unknown fail_on/);
  await expect(runWasm(page, TINY, '', 'auto', 'standard', 'true', '', 'high', 'yaml'))
    .rejects.toThrow(/unknown output/);
  // A package.json pasted into the lockfile field is called out, not silently ignored.
  await expect(runWasm(page, TINY, TINY)).rejects.toThrow(/looks like a package.json/);

  // Cap boundary: exactly at the limit gets past the size check (and fails on
  // content instead); one byte over is rejected as too large.
  const atCap = await page.evaluate(() => 'x'.repeat(2 * 1024 * 1024));
  const overCap = await page.evaluate(() => 'x'.repeat(2 * 1024 * 1024 + 1));
  await expect(runWasm(page, atCap)).rejects.toThrow(/could not detect the input format/);
  await expect(runWasm(page, overCap)).rejects.toThrow(/manifest is too large/);
});

test('dependency-risk-auditor page renders the report and reacts to controls', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.fill('#in-manifest', TINY);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('DEPENDENCY RISK AUDIT — FAIL', { timeout: 15_000 });
  expect((await out.textContent())!.trim()).toBe(TINY_TEXT.trim());

  // Strictness select adds the low/info rules.
  await page.selectOption('#in-strictness', 'strict');
  await expect(out).toContainText('[no-lockfile-supplied]', { timeout: 15_000 });

  // include_dev renders as a checkbox and defaults to checked (descriptor default true).
  const includeDev = page.locator('#in-include_dev');
  await expect(includeDev).toBeChecked();
  await page.fill('#in-manifest', '{"engines":{"node":">=20"},"devDependencies":{"b":"*"}}');
  await page.selectOption('#in-strictness', 'standard');
  await expect(out).toContainText('[wildcard-version] b (devDependencies)', { timeout: 15_000 });
  await page.uncheck('#in-include_dev');
  await expect(out).toContainText('No findings at this strictness level.', { timeout: 15_000 });

  // Markdown output.
  await page.check('#in-include_dev');
  await page.fill('#in-manifest', TINY);
  await page.selectOption('#in-output', 'markdown');
  await expect(out).toContainText('| Severity | Rule | Subject | Value | Detail |', { timeout: 15_000 });
});

test('dependency-risk-auditor page pre-fills and computes from a deep link', async ({ page }) => {
  const qs = new URLSearchParams({
    manifest: MESSY,
    manifest_format: 'package-json',
    strictness: 'strict',
    include_dev: 'false',
    ignore: 'range-prefix',
    fail_on: 'never',
    output: 'json',
  });
  await page.goto(`/tools/dependency-risk-auditor/?${qs.toString()}`);

  await expect(page.locator('#in-manifest')).toHaveValue(MESSY, { timeout: 15_000 });
  await expect(page.locator('#in-manifest_format')).toHaveValue('package-json');
  await expect(page.locator('#in-strictness')).toHaveValue('strict');
  await expect(page.locator('#in-include_dev')).not.toBeChecked();
  await expect(page.locator('#in-ignore')).toHaveValue('range-prefix');
  await expect(page.locator('#in-fail_on')).toHaveValue('never');
  await expect(page.locator('#in-output')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"detected_format": "package-json"', { timeout: 15_000 });
  const json = JSON.parse((await out.textContent())!);
  expect(json.verdict).toBe('pass');
  expect(json.strictness).toBe('strict');
  const rules = json.findings.map((f: { rule: string }) => f.rule);
  expect(rules).toContain('missing-engines');
  expect(rules).not.toContain('range-prefix');
  // include_dev=false drops the devDependencies duplicate.
  expect(rules).not.toContain('duplicate-dependency');
});

test('dependency-risk-auditor page ships a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/dependency-risk-auditor/');
  await page.waitForSelector('#in-manifest');

  const cli = (await page.locator('pre.tool-cli-code code').first().textContent())!.trim();
  expect(cli.startsWith('gizza tool dependency-risk-auditor ')).toBe(true);

  // The example's argument must be real, runnable input — not a prose placeholder.
  const arg = cli.slice('gizza tool dependency-risk-auditor '.length).trim().replace(/^'|'$/g, '');
  const report = await runWasm(page, arg);
  expect(report).toContain('DEPENDENCY RISK AUDIT');
  expect(report).toContain('Input: package.json');
});
