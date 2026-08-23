## About this tool

**Kubernetes Manifest Generator** turns the handful of details you actually know about an app — its
name, image, port — into a complete, validated manifest: an `apps/v1` Deployment and a matching
`v1` Service, emitted as two YAML documents separated by `---` and ready for `kubectl apply -f -`.
Everything runs in your browser through WebAssembly, so image names, env vars, and internal
hostnames never leave your machine.

The two resources are wired together for you: the pod template carries an `app: <name>` label, the
Deployment selector and the Service selector both match it, and the Service `targetPort` points at
the container port. That is the part people usually get subtly wrong by hand — a selector that
matches nothing produces a Service with no endpoints and no error message.

### Options

- **Replicas** — 0–100, default 1. Zero is valid and scales the Deployment down to no pods.
- **Container port / Service port** — the `containerPort` the app listens on, and the port the
  Service exposes. The Service `targetPort` always follows the container port, and probes use it too.
- **Service type** — `ClusterIP` (default, reachable only inside the cluster), `NodePort` (a port on
  every node), or `LoadBalancer` (asks your cloud provider for one).
- **nodePort** — pin a specific port in the 30000–32767 range. Only valid with `NodePort`; leave it
  blank and Kubernetes assigns one.
- **Image pull policy** — `IfNotPresent` (default), `Always`, or `Never`.
- **CPU / memory requests and limits** — standard Kubernetes quantities (`100m`, `0.5`, `128Mi`,
  `1Gi`). Each is dropped when blank, and the whole `resources:` block disappears if all four are.
- **Environment variables** — `KEY=value`, one per line. `#` comments and blank lines are skipped, a
  leading `export ` is stripped, and quoted values are unquoted.
- **Extra labels** — `key=value`, newline- or comma-separated, merged after the standard `app` label
  on the Deployment, the Service, and the pod template. `app` itself is reserved.
- **Health probe path** — one HTTP path (`/healthz`) gives you both a `livenessProbe` and a
  `readinessProbe` on the container port.

### Example

Name `web`, image `nginx:1.27`, container port 8080, service port 80:

```yaml
apiVersion: apps/v1
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
      protocol: TCP
```

## FAQ

<details>
<summary>Can I apply the output directly with kubectl?</summary>

Yes. Copy the manifest into a file and run `kubectl apply -f manifest.yaml`, or paste it into
`kubectl apply -f -`. Both documents are applied in order, so the Deployment and Service are created
together.

</details>

<details>
<summary>Why is my Service not reaching any pods?</summary>

Almost always a selector mismatch — but not here: this tool derives the pod labels, the Deployment
`matchLabels`, and the Service `selector` from the same **App name**, so they cannot drift. If
endpoints are still empty, check that the container is actually listening on the **container port**
you entered, since the Service `targetPort` is wired to it.

</details>

<details>
<summary>What resource quantities are accepted?</summary>

Kubernetes' own syntax. CPU takes plain numbers or millicores — `2`, `0.5`, `100m`. Memory takes
plain bytes or a binary/decimal suffix — `134217728`, `128Mi`, `512M`, `1Gi`. Anything else is
rejected with a message rather than silently written into the manifest.

</details>

<details>
<summary>When should I set a nodePort?</summary>

Only when **Service type** is `NodePort` and you need a stable, known port — for example a firewall
rule or an external load balancer already points at it. The value must be in the cluster's default
30000–32767 range. Otherwise leave it blank and let Kubernetes allocate one.

</details>

<details>
<summary>Does one probe path really configure both probes?</summary>

Yes. The path you enter is used for a `livenessProbe` (`initialDelaySeconds: 15`) and a
`readinessProbe` (`initialDelaySeconds: 5`), both `httpGet` on the container port with
`periodSeconds: 10`. Leave the field blank and neither probe is emitted. Tune the timings in the
generated YAML if your app starts slowly.

</details>

<details>
<summary>Why are some values wrapped in quotes?</summary>

Because YAML would otherwise change their type. An app named `123`, a label value of `true`, or an
env var `PORT=8080` are all strings to Kubernetes, so they are emitted double-quoted. Env values are
always quoted for that reason. Unambiguous values such as `nginx:1.27` and `/healthz` stay bare.

</details>
