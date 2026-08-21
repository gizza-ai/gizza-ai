import { test, expect } from './fixtures';

const SAMPLE =
  'apiVersion: v1\n' +
  'kind: Service\n' +
  'metadata:\n' +
  '  name: web\n' +
  '  namespace: prod\n' +
  'spec:\n' +
  '  ports:\n' +
  '    - port: 80\n' +
  '---\n' +
  'apiVersion: apps/v1\n' +
  'kind: Deployment\n' +
  'metadata:\n' +
  '  name: web\n' +
  'spec:\n' +
  '  replicas: 2\n';

const INDEX_OUTPUT =
  '#  KIND        NAME  NAMESPACE  APIVERSION  LINES  FILE\n' +
  '1  Service     web   prod       v1          8      service-web.yaml\n' +
  '2  Deployment  web   -          apps/v1     6      deployment-web.yaml\n' +
  '\n' +
  '2 documents, 2 kinds\n';

async function runWasm(
  page: any,
  manifest = SAMPLE,
  output = 'files',
  filenameTemplate = '{kind}-{name}.yaml',
  include = '',
  exclude = '',
  sort = 'document',
  skipNonK8s = 'false',
  expandLists = 'true',
  includeTripleDash = 'false',
): Promise<string> {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/k8s-manifest-splitter/gizza_ai_k8s_manifest_splitter_web.js');
    await mod.default('/tools/k8s-manifest-splitter/gizza_ai_k8s_manifest_splitter_web_bg.wasm');
    return mod.run(
      args.manifest,
      args.output,
      args.filenameTemplate,
      args.include,
      args.exclude,
      args.sort,
      args.skipNonK8s,
      args.expandLists,
      args.includeTripleDash,
    );
  }, {
    manifest,
    output,
    filenameTemplate,
    include,
    exclude,
    sort,
    skipNonK8s,
    expandLists,
    includeTripleDash,
  });
}

test('k8s-manifest-splitter wasm emits exact index output', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-splitter/');
  await page.waitForSelector('#in-manifest');

  await expect(runWasm(page, SAMPLE, 'index')).resolves.toBe(INDEX_OUTPUT);
});

test('k8s-manifest-splitter wasm covers output choices, sort choices and booleans', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-splitter/');
  await page.waitForSelector('#in-manifest');

  expect(await runWasm(page, SAMPLE, 'files')).toContain('# ===== service-web.yaml =====');
  expect(JSON.parse(await runWasm(page, SAMPLE, 'json'))[1]).toMatchObject({
    file: 'deployment-web.yaml',
    kind: 'Deployment',
    name: 'web',
  });
  expect(await runWasm(page, SAMPLE, 'kustomization')).toBe(
    'apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - service-web.yaml\n  - deployment-web.yaml\n',
  );
  expect(await runWasm(page, SAMPLE, 'shell', 'manifests/{kind}-{name}.yaml')).toContain(
    "mkdir -p \"$(dirname 'manifests/service-web.yaml')\"",
  );

  const applyInput =
    'apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n---\n' +
    'apiVersion: v1\nkind: Namespace\nmetadata:\n  name: prod\n---\n' +
    'apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n';
  expect(await runWasm(page, applyInput, 'kustomization', '{kind}-{name}.yaml', '', '', 'apply'))
    .toContain('  - namespace-prod.yaml\n  - configmap-cfg.yaml\n  - deployment-web.yaml\n');
  expect(await runWasm(page, SAMPLE, 'index', '{kind}-{name}.yaml', '', '', 'kind')).toContain('Deployment');
  expect(await runWasm(page, SAMPLE, 'index', '{kind}-{name}.yaml', '', '', 'name')).toContain('service-web.yaml');

  const mixed = 'note: not kubernetes\n---\n' + SAMPLE;
  expect(await runWasm(page, mixed, 'files', '{kind}-{name}.yaml', '', '', 'document', 'true'))
    .not.toContain('unknown-unnamed.yaml');
  expect(await runWasm(page, SAMPLE, 'files', '{kind}-{name}.yaml', '', '', 'document', 'false', 'true', 'true'))
    .toContain('# ===== service-web.yaml =====\n---\napiVersion: v1');

  const list = 'apiVersion: v1\nkind: List\nitems:\n  - apiVersion: v1\n    kind: Service\n    metadata:\n      name: a\n';
  expect(await runWasm(page, list, 'index', '{kind}-{name}.yaml', '', '', 'document', 'false', 'false'))
    .toContain('List');
});

test('k8s-manifest-splitter wasm covers filters, templates, boundaries and errors', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-splitter/');
  await page.waitForSelector('#in-manifest');

  await expect(runWasm(page, SAMPLE, 'index', '{namespace}/{kind}-{name}.yaml', 'Deployment,Service/web', 'Service/*'))
    .resolves.toContain('default/deployment-web.yaml');

  const labelled =
    'apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n  labels:\n    app: storefront\nspec:\n  replicas: 3\n';
  await expect(runWasm(page, labelled, 'kustomization', '{metadata.labels.app}-{spec.replicas}.yaml'))
    .resolves.toContain('  - storefront-3.yaml');
  await expect(runWasm(page, SAMPLE, 'files', '{metadata.labels.app}.yaml'))
    .rejects.toThrow(/metadata\.labels\.app/);

  const doc = 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: c\n';
  await expect(runWasm(page, Array(1000).fill(doc).join('---\n'), 'index'))
    .resolves.toContain('1000 documents');
  await expect(runWasm(page, Array(1001).fill(doc).join('---\n'), 'index'))
    .rejects.toThrow(/the limit is 1000/);
});

test('k8s-manifest-splitter page renders exact output and reacts to controls', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-splitter/');
  await page.fill('#in-manifest', SAMPLE);
  await page.selectOption('#in-output', 'index');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 documents, 2 kinds', { timeout: 15_000 });
  expect(await out.textContent()).toBe(INDEX_OUTPUT);

  await page.selectOption('#in-output', 'kustomization');
  await page.fill('#in-filename_template', '{namespace}/{kind}-{name}.yaml');
  await expect(out).toContainText('prod/service-web.yaml', { timeout: 15_000 });
  await expect(out).toContainText('default/deployment-web.yaml');
});

test('k8s-manifest-splitter deep link pre-fills params and computes', async ({ page }) => {
  const params = new URLSearchParams({
    manifest: SAMPLE,
    output: 'kustomization',
    filename_template: '{namespace}/{kind}-{name}.yaml',
    include: '',
    exclude: '',
    sort: 'apply',
    skip_non_k8s: 'false',
    expand_lists: 'true',
    include_triple_dash: 'false',
  });
  await page.goto(`/tools/k8s-manifest-splitter/?${params.toString()}`);

  await expect(page.locator('#in-manifest')).toHaveValue(SAMPLE, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('kustomization');
  await expect(page.locator('#in-sort')).toHaveValue('apply');
  await expect(page.locator('#tool-output')).toContainText('prod/service-web.yaml', { timeout: 15_000 });
});

test('k8s-manifest-splitter page ships a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-splitter/');
  await page.waitForSelector('#in-manifest');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool k8s-manifest-splitter');
  expect(cli).toContain('apiVersion: v1');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
