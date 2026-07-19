## About this tool

**env to ConfigMap / Secret** turns a `.env` file into a ready-to-apply
Kubernetes manifest. Paste your `KEY=value` lines and get back clean YAML:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: app-config
data:
  DB_HOST: localhost
  DB_PORT: "5432"
  LOG_LEVEL: info
```

- **ConfigMap or Secret.** Pick *Secret* for confidential values — each value is
  **base64-encoded** under `data`, or kept plaintext under `stringData` (which
  Kubernetes encodes when you apply it). Secrets are emitted as `type: Opaque`.
- **Values stay strings.** Anything that looks like a number or boolean (`5432`,
  `true`) is quoted, so `kubectl apply` doesn't reject it with a type error.
- **Metadata you control.** Set `metadata.name`, an optional `namespace`, and
  optional `labels` (`app=web,tier=backend`).
- **`.env` niceties.** `#` comment lines, blank lines and a leading `export ` are
  ignored; quoted values are unquoted; duplicate keys keep the last value.

### Worked example

Input (a Secret):

```
API_TOKEN=s3cr3t
DB_PASSWORD=hunter2
```

Output with **kind = Secret**, **name = app-secrets**:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: app-secrets
type: Opaque
data:
  API_TOKEN: czNjcjN0
  DB_PASSWORD: aHVudGVyMg==
```

### Privacy

Everything runs **in your browser** via WebAssembly — your `.env` file, including
any secrets, is never uploaded. Also available from the [gizza CLI](/) and in chat.

### Common uses

- Bootstrap a ConfigMap or Secret from an existing local `.env` file.
- Generate manifests to commit to a GitOps repo (use `stringData` for readable
  diffs, or `data` to match `kubectl` output).

## FAQ

<details>
<summary>Why are some values wrapped in quotes?</summary>

In a ConfigMap or a `stringData` Secret, every value must be a string. YAML would
otherwise read `PORT: 5432` as an integer and `DEBUG: true` as a boolean, and
`kubectl apply` rejects those with a "cannot convert" error. So any value that
looks like a number, boolean (`true`/`false`/`yes`/`no`/`on`/`off`), or null is
double-quoted to keep it a string. Plain identifiers like `localhost` are left
unquoted.

</details>

<details>
<summary>What's the difference between data and stringData for a Secret?</summary>

Both end up identical in the cluster. `data` stores each value **base64-encoded**
— that's how Kubernetes persists Secrets and what `kubectl get secret -o yaml`
shows. `stringData` is a write-only convenience field: you provide plaintext and
Kubernetes base64-encodes it on apply, which keeps your committed manifests
readable. Base64 is **not** encryption — anyone who can read the Secret can decode
it.

</details>

<details>
<summary>How are comments, quotes and duplicates handled?</summary>

Lines starting with `#` and blank lines are skipped, and a leading `export ` is
dropped. A value wrapped in single or double quotes is unquoted (double quotes
also interpret `\n`, `\t`, `\\` and `\"`), and an unquoted trailing ` # comment`
is removed. If a key appears more than once, the last value wins.

</details>

<details>
<summary>Does this cover Docker-registry, TLS or basic-auth Secrets?</summary>

No — this tool produces `Opaque` Secrets, the direct equivalent of
`kubectl create secret generic --from-env-file`. The typed Secrets
(`kubernetes.io/dockerconfigjson`, `kubernetes.io/tls`,
`kubernetes.io/basic-auth`) expect specific fixed keys and file contents rather
than a flat `.env`, so they're out of scope here.

</details>

<details>
<summary>Can I set a namespace and labels?</summary>

Yes. Fill the optional **Namespace** field to add `metadata.namespace` (leave it
blank to omit it and apply to your current namespace). Add **labels** as
comma-separated `key=value` pairs, e.g. `app=web,tier=backend`, and they're
emitted under `metadata.labels`.

</details>
