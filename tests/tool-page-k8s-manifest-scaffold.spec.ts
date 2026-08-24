import { test, expect } from './fixtures';

// Expected manifests are byte-for-byte what `gizza tool k8s-manifest-scaffold`
// prints for the same params — the page and the CLI share the same core crate,
// so any drift between them is a real bug.

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const MINIMAL = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  labels:
    app: web
spec:
  replicas: 1
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
        - name: web
          image: nginx:1.27
          imagePullPolicy: IfNotPresent
          ports:
            - name: http
              containerPort: 8080
              protocol: TCP
---
apiVersion: v1
kind: Service
metadata:
  name: web
  labels:
    app: web
spec:
  type: ClusterIP
  selector:
    app: web
  ports:
    - name: http
      port: 80
      targetPort: 8080
      protocol: TCP`;

const NODEPORT = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: dashboard
  labels:
    app: dashboard
spec:
  replicas: 1
  selector:
    matchLabels:
      app: dashboard
  template:
    metadata:
      labels:
        app: dashboard
    spec:
      containers:
        - name: dashboard
          image: grafana/grafana:11.1.0
          imagePullPolicy: IfNotPresent
          ports:
            - name: http
              containerPort: 3000
              protocol: TCP
---
apiVersion: v1
kind: Service
metadata:
  name: dashboard
  labels:
    app: dashboard
spec:
  type: NodePort
  selector:
    app: dashboard
  ports:
    - name: http
      port: 3000
      targetPort: 3000
      protocol: TCP
      nodePort: 30080`;

const PRODUCTION = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: prod
  labels:
    app: api
    tier: backend
    team: payments
spec:
  replicas: 3
  selector:
    matchLabels:
      app: api
  template:
    metadata:
      labels:
        app: api
        tier: backend
        team: payments
    spec:
      containers:
        - name: api
          image: ghcr.io/acme/api:v1.2.3
          imagePullPolicy: Always
          ports:
            - name: http
              containerPort: 8000
              protocol: TCP
          env:
            - name: LOG_LEVEL
              value: "info"
            - name: PORT
              value: "8000"
          resources:
            requests:
              cpu: 100m
              memory: 128Mi
            limits:
              cpu: 500m
              memory: 256Mi
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8000
            initialDelaySeconds: 15
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /healthz
              port: 8000
            initialDelaySeconds: 5
            periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: api
  namespace: prod
  labels:
    app: api
    tier: backend
    team: payments
spec:
  type: ClusterIP
  selector:
    app: api
  ports:
    - name: http
      port: 80
      targetPort: 8000
      protocol: TCP`;

test('k8s-manifest-scaffold page emits a Deployment + Service from name and image', async ({
  page,
}) => {
  await page.goto('/tools/k8s-manifest-scaffold/');
  await setField(page, '#in-name', 'web');
  await setField(page, '#in-image', 'nginx:1.27');

  await expect(page.locator('#tool-output')).toContainText('kind: Service', { timeout: 15_000 });
  expect(await outputText(page)).toBe(MINIMAL);
});

test('k8s-manifest-scaffold deep link renders namespace, resources, env, labels and probes', async ({
  page,
}) => {
  const params = new URLSearchParams({
    name: 'api',
    image: 'ghcr.io/acme/api:v1.2.3',
    namespace: 'prod',
    replicas: '3',
    container_port: '8000',
    service_port: '80',
    service_type: 'ClusterIP',
    node_port: '',
    image_pull_policy: 'Always',
    cpu_request: '100m',
    cpu_limit: '500m',
    memory_request: '128Mi',
    memory_limit: '256Mi',
    env: 'LOG_LEVEL=info\nPORT=8000',
    labels: 'tier=backend,team=payments',
    probe_path: '/healthz',
  });
  await page.goto(`/tools/k8s-manifest-scaffold/?${params.toString()}`);

  await expect(page.locator('#in-namespace')).toHaveValue('prod', { timeout: 15_000 });
  await expect(page.locator('#in-replicas')).toHaveValue('3');
  await expect(page.locator('#in-image_pull_policy')).toHaveValue('Always');
  await expect(page.locator('#in-probe_path')).toHaveValue('/healthz');

  await expect(page.locator('#tool-output')).toContainText('readinessProbe', { timeout: 15_000 });
  expect(await outputText(page)).toBe(PRODUCTION);
});

test('k8s-manifest-scaffold NodePort service pins the nodePort', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-scaffold/');
  await setField(page, '#in-name', 'dashboard');
  await setField(page, '#in-image', 'grafana/grafana:11.1.0');
  await setField(page, '#in-container_port', '3000');
  await setField(page, '#in-service_port', '3000');
  await page.selectOption('#in-service_type', 'NodePort');
  await setField(page, '#in-node_port', '30080');

  await expect(page.locator('#tool-output')).toContainText('nodePort: 30080', { timeout: 15_000 });
  expect(await outputText(page)).toBe(NODEPORT);

  // A pinned nodePort only makes sense for a NodePort Service.
  await page.selectOption('#in-service_type', 'ClusterIP');
  await expect(page.locator('#tool-output')).toContainText(
    'node_port 30080 is only valid when service_type is "NodePort"',
    { timeout: 15_000 },
  );
});

test('k8s-manifest-scaffold enforces the replica range at both ends', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-scaffold/');
  await setField(page, '#in-name', 'edge');
  await setField(page, '#in-image', 'nginx:1.27');

  await setField(page, '#in-replicas', '0');
  await expect(page.locator('#tool-output')).toContainText('replicas: 0', { timeout: 15_000 });

  await setField(page, '#in-replicas', '100');
  await expect(page.locator('#tool-output')).toContainText('replicas: 100', { timeout: 15_000 });
  expect(await outputText(page)).toContain('kind: Service');

  await setField(page, '#in-replicas', '101');
  await expect(page.locator('#tool-output')).toContainText(
    'invalid replicas 101: use a whole number from 0 to 100',
    { timeout: 15_000 },
  );
});

test('k8s-manifest-scaffold shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/k8s-manifest-scaffold/');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool k8s-manifest-scaffold');
  expect(cli).toContain('image=nginx:1.27');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
