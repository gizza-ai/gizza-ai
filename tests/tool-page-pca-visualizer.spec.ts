import { test, expect } from './fixtures';

const tool = '/tools/pca-visualizer/';
const data = 'sepal_len,sepal_wid,petal_len,petal_wid,species\n5.1,3.5,1.4,0.2,setosa\n4.9,3.0,1.4,0.2,setosa\n5.8,2.7,4.1,1.0,versicolor\n6.4,3.2,4.5,1.5,versicolor\n6.5,3.0,5.8,2.2,virginica\n7.6,3.0,6.6,2.1,virginica';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  dataText: string,
  method = 'pca',
  labelColumn = 'species',
  scale = 'true',
  perplexity = '30',
  iterations = '500',
  learningRate = '200',
  showLabels = 'false',
  pointSize = '4',
  title = '',
  width = '720',
  height = '520',
  format = 'svg',
): Promise<string> {
  return await page.evaluate(
    async ({ dataText, method, labelColumn, scale, perplexity, iterations, learningRate, showLabels, pointSize, title, width, height, format }) => {
      const mod = await import('/tools/pca-visualizer/gizza_ai_pca_visualizer_web.js');
      await mod.default('/tools/pca-visualizer/gizza_ai_pca_visualizer_web_bg.wasm');
      return mod.run(dataText, method, labelColumn, scale, perplexity, iterations, learningRate, showLabels, pointSize, title, width, height, format);
    },
    { dataText, method, labelColumn, scale, perplexity, iterations, learningRate, showLabels, pointSize, title, width, height, format },
  );
}

test('pca-visualizer page renders an SVG scatter plot with labels and legend', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-data'), data);
  await page.fill('#in-title', 'Iris PCA');
  await page.fill('#in-point_size', '5');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15_000 });
  await expect(out).toContainText('Iris PCA');
  await expect(out).toContainText('PC1');
  await expect(out).toContainText('setosa');
  await expect(out).toContainText('<circle');
});

test('pca-visualizer deep link pre-fills t-SNE JSON settings', async ({ page }) => {
  const qs = new URLSearchParams({
    data,
    method: 'tsne',
    label_column: 'species',
    scale: 'false',
    perplexity: '3',
    iterations: '300',
    learning_rate: '120',
    show_labels: 'true',
    point_size: '6',
    title: 'Iris t-SNE',
    width: '640',
    height: '420',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15_000 });
  await expect(page.locator('#in-method')).toHaveValue('tsne');
  await expect(page.locator('#in-label_column')).toHaveValue('species');
  await expect(page.locator('#in-scale')).not.toBeChecked();
  await expect(page.locator('#in-show_labels')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"method"');
  await expect(page.locator('#tool-output')).toContainText('"tsne"');
});

test('pca-visualizer wasm covers methods, formats, boundary, checkbox, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const svg = await runWasm(page, data, 'pca', 'species', 'true', '30', '500', '200', 'true', '8', 'Labelled PCA', '300', '200', 'svg');
  expect(svg).toContain('<svg');
  expect(svg).toContain('Labelled PCA');
  expect(svg).toContain('>setosa</text>');
  expect((svg.match(/<circle/g) || []).length).toBe(9); // 6 points + 3 legend entries.

  const csv = await runWasm(page, data, 'pca', '5', 'false', '30', '500', '200', 'false', '4', '', '720', '520', 'csv');
  expect(csv).toContain('index,label,pc1,pc2');
  expect(csv).toContain('1,setosa,');

  const json = JSON.parse(await runWasm(page, data, 'tsne', 'species', 'true', '3', '300', '120', 'false', '4', '', '720', '520', 'json'));
  expect(json.method).toBe('tsne');
  expect(json.points).toHaveLength(6);
  expect(json.perplexity_used).toBeCloseTo(5 / 3, 6);

  await expect(runWasm(page, '', 'pca')).rejects.toThrow(/no data/);
  await expect(runWasm(page, data, 'umap')).rejects.toThrow(/unknown method/);
  await expect(runWasm(page, data, 'pca', 'missing')).rejects.toThrow(/no column named/);
  await expect(runWasm(page, data, 'pca', 'species', 'true', '30', '500', '200', 'false', '0')).rejects.toThrow(/point_size must be between 1 and 20/);
  await expect(runWasm(page, data, 'pca', 'species', 'true', '30', '500', '200', 'false', '4', '', '299')).rejects.toThrow(/width must be between 300 and 2000/);
});

test('pca-visualizer ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Iris-like PCA plot',
    't-SNE clusters',
    'Coordinates as CSV',
  ]);
});
