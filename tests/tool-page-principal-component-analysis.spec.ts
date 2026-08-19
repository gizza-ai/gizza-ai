import { test, expect } from './fixtures';

const DATA = 'height,weight,age\n170,65,30\n180,80,42\n165,59,25\n175,72,35\n190,95,50\n160,54,22';

// Exact formatted report for DATA with every component kept, standardized
// (correlation-matrix PCA) — the default settings.
const TEXT_REPORT = `PCA on 6 observations × 3 variables (correlation matrix — columns standardized to unit variance)

Explained variance:
  component   eigenvalue   proportion    percent   cumulative
  PC1           2.992685     0.997562   99.7562%     0.997562
  PC2           0.004537     0.001512    0.1512%     0.999074
  PC3           0.002778     0.000926    0.0926%            1

Total variance: 3
Components needed: 1 for 90%, 1 for 95%, 1 for 99% of the variance.
Kaiser criterion (eigenvalue > 1): 1 component.

Scree plot (share of the total variance):
  PC1  ████████████████████  99.7562%
  PC2  █                      0.1512%
  PC3  █                      0.0926%

Loadings (weight of each variable in each component):
  variable        PC1         PC2         PC3
  height     0.577321   -0.625402    -0.52495
  weight      0.57751   -0.141739    0.803985
  age         0.57722    0.767321   -0.279347

Scores (each observation projected onto the 3 components):
  row         PC1         PC2         PC3
  1      -0.62035   -0.041425   -0.045008
  2      1.144526    0.106453   -0.043872
  3     -1.390638    -0.05715    0.008408
  4      0.188408   -0.035141   -0.044869
  5      2.691621   -0.035173    0.062659
  6     -2.013568    0.062435    0.062682`;

// Scores only, top 2 components.
const SCORES_CSV = `row,PC1,PC2
1,-0.62035,-0.041425
2,1.144526,0.106453
3,-1.390638,-0.05715
4,0.188408,-0.035141
5,2.691621,-0.035173
6,-2.013568,0.062435`;

// components=1 + scale=false: covariance-matrix PCA, one component reported.
const COVARIANCE_PC1 = `PCA on 6 observations × 3 variables (covariance matrix — columns centered only)

Explained variance:
  component   eigenvalue   proportion    percent   cumulative
  PC1         453.438695     0.997811   99.7811%     0.997811

Total variance: 454.433333
Components needed: 1 for 90%, 1 for 95%, 1 for 99% of the variance.

Scree plot (share of the total variance):
  PC1  ████████████████████  99.7811%

Loadings (weight of each variable in each component):
  variable        PC1
  height     0.506482
  weight     0.704615
  age        0.496984

Scores (each observation projected onto the 1 component):
  row          PC1
  1      -7.786466
  2      13.811394
  3     -17.031489
  4       2.163172
  5      33.421316
  6     -24.577928`;

async function runWasm(
  page: any,
  data = DATA,
  labels = '',
  components = '0',
  scale = 'true',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/principal-component-analysis/gizza_ai_principal_component_analysis_web.js');
    await mod.default('/tools/principal-component-analysis/gizza_ai_principal_component_analysis_web_bg.wasm');
    return mod.run(args.data, args.labels, args.components, args.scale, args.format);
  }, { data, labels, components, scale, format });
}

test('principal-component-analysis page reports variance, scree, loadings and scores', async ({ page }) => {
  await page.goto('/tools/principal-component-analysis/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-components', '0');
  await page.selectOption('#in-format', 'text');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('PCA on 6 observations \u00d7 3 variables', { timeout: 20_000 });
  await expect(output).toContainText('correlation matrix \u2014 columns standardized to unit variance');
  await expect(output).toContainText('Total variance: 3');
  await expect(output).toContainText('Components needed: 1 for 90%, 1 for 95%, 1 for 99% of the variance.');
  await expect(output).toContainText('Kaiser criterion (eigenvalue > 1): 1 component.');
  await expect(output).toContainText('Scree plot (share of the total variance):');
  // Header row names the loadings, not v1/v2/v3.
  await expect(output).toContainText('height');
  await expect(output).toContainText('99.7562%');

  // The scree bar is a real bar: PC1 holds ~all the variance, so it fills the
  // width while PC2/PC3 get the minimum visible mark.
  const report = (await output.textContent())!;
  expect(report).toContain(`PC1  ${'\u2588'.repeat(20)}  99.7562%`);
  expect(report).toContain(`PC2  \u2588${' '.repeat(19)}   0.1512%`);
});

test('principal-component-analysis deep link applies components=1 and scale=false', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    labels: '',
    components: '1',
    scale: 'false',
    format: 'text',
  });
  await page.goto(`/tools/principal-component-analysis/?${params.toString()}`);

  // scale defaults to CHECKED, so this exercises the non-default state.
  await expect(page.locator('#in-scale')).not.toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-components')).toHaveValue('1');
  await expect(page.locator('#in-format')).toHaveValue('text');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('covariance matrix \u2014 columns centered only', { timeout: 20_000 });
  await expect(output).toContainText('Total variance: 454.433333');
  // Only one component is reported...
  await expect(output).toContainText('Scores (each observation projected onto the 1 component):');
  await expect(output).not.toContainText('PC2');
  // ...and the Kaiser line is suppressed on unstandardized data.
  await expect(output).not.toContainText('Kaiser criterion');
});

test('principal-component-analysis wasm covers formats, the component cap and errors', async ({ page }) => {
  await page.goto('/tools/principal-component-analysis/');
  await page.waitForSelector('#in-data');

  // Exact text report, byte for byte.
  expect(await runWasm(page)).toBe(TEXT_REPORT);

  // Same numbers through the deep-link combination.
  expect(await runWasm(page, DATA, '', '1', 'false', 'text')).toBe(COVARIANCE_PC1);

  // csv enum: scores only, one header row plus one row per observation.
  expect(await runWasm(page, DATA, '', '2', 'true', 'csv')).toBe(SCORES_CSV);

  // json enum: the full structured result, including what you need to project
  // new observations later.
  const json = JSON.parse(await runWasm(page, DATA, '', '0', 'true', 'json'));
  expect(json.n).toBe(6);
  expect(json.variables).toBe(3);
  expect(json.variable_names).toEqual(['height', 'weight', 'age']);
  expect(json.standardized).toBe(true);
  expect(json.total_variance).toBe(3);
  expect(json.components).toHaveLength(3);
  expect(json.components[0].name).toBe('PC1');
  expect(json.components[0].eigenvalue).toBe(2.992685);
  expect(json.components[0].proportion).toBe(0.997562);
  expect(json.components[0].loadings).toHaveLength(3);
  expect(json.kaiser_components).toBe(1);
  expect(json.components_for_90).toBe(1);
  expect(json.means).toEqual([173.333333, 70.833333, 34]);
  expect(json.scores).toHaveLength(6);

  // Explicit labels override the header row.
  const labelled = JSON.parse(await runWasm(page, DATA, 'a,b,c', '1', 'true', 'json'));
  expect(labelled.variable_names).toEqual(['a', 'b', 'c']);

  // components cap: 100 is the boundary and is accepted (clamped to the 3
  // available components); 101 is rejected.
  const capped = await runWasm(page, DATA, '', '100', 'true', 'text');
  expect(capped).toBe(TEXT_REPORT);
  await expect(runWasm(page, DATA, '', '101', 'true', 'text'))
    .rejects.toThrow(/components must be at most 100/);

  // Other input errors surface as rejections, not silent output.
  await expect(runWasm(page, DATA, '', '1.5', 'true', 'text'))
    .rejects.toThrow(/whole number/);
  await expect(runWasm(page, '1,5\n2,5\n3,5', '', '0', 'true', 'text'))
    .rejects.toThrow(/constant/);
  await expect(runWasm(page, '1,2\n3,4,5', '', '0', 'true', 'text'))
    .rejects.toThrow(/same number of columns/);
  await expect(runWasm(page, DATA, '', '0', 'true', 'xml'))
    .rejects.toThrow(/unknown format/);
});

test('principal-component-analysis generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/principal-component-analysis/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool principal-component-analysis');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
