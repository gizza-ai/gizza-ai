## About this tool

**Docker Compose Validator** checks a `docker-compose.yml` or `compose.yaml` for the errors that
ordinary YAML validators cannot see. The file can be perfectly valid YAML and still fail when
Compose reads it because a service depends on a name that does not exist, a named volume was never
declared, a port is outside the valid range, or two services publish the same host port.

Paste the file, choose a preset, and get a line-and-column report with rule ids and remediation
text. The default preset reports compose-breaking errors plus common warnings such as obsolete
`version:` keys, floating image tags, `privileged: true`, host networking, hard-coded environment
secrets and unquoted port mappings. The strict preset adds hardening hints for healthchecks,
restart policies, resource limits, logging options and broad host-port binds.

Everything runs locally in WebAssembly. No Compose file, image name, secret-looking environment
value, hostname or port list is uploaded.

### Worked example

Input:

```yaml
services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
    depends_on:
      - api
    volumes:
      - data:/usr/share/nginx/html
```

Readable report output:

```text
INVALID — 1 service, 0 networks, 0 volumes
preset default — 2 errors, 1 warning, 0 hints

3:5  warning  image-tag             service 'web' pins image tag ':latest', which moves without warning — use a version tag or an image digest
7:9  error    undefined-depends-on  service 'web' depends on 'api', which is not defined under 'services:' — defined services are: web
9:9  error    undefined-volume      service 'web' mounts named volume 'data', which is not declared under the top-level 'volumes:' key — no top-level 'volumes:' key exists yet
```

Switch **Output format** to JSON when you want CI to parse counts, rule ids and exact source
locations. Use **Disable rules** for deliberate exceptions such as `image-tag` in throwaway local
stacks.

### Presets and filters

| Control | Use it when |
| --- | --- |
| `essential` preset | You only want findings that can break `docker compose up`: syntax, structure, undefined references, invalid ports, dependency cycles and duplicates. |
| `default` preset | You also want practical warnings about deprecated, insecure or non-repeatable compose patterns. |
| `strict` preset | You are reviewing production-ish files and want hardening hints too. |
| `strict_warnings` | Your CI treats warnings as build failures. Hints stay hints. |
| `min_severity` | You want a shorter report, for example only errors in a pre-commit hook. |

### Limits and edge cases

- Maximum input size is 1 MiB. Compose files are configuration, not data sets; larger input is
  rejected instead of freezing the page.
- The report is capped at 500 findings and tells you if additional findings were omitted.
- Only the first YAML document is analysed.
- YAML anchors, aliases and `<<` merge keys are resolved before service checks run.
- Short and long port syntax are checked, including ranges, protocols, host IPs and bracketed IPv6
  hosts. Very wide ranges are validated without expanding every port for duplicate checks.
- Bind mounts are distinguished from named volumes by path-like sources such as `/data`, `./data`,
  `../data`, `~/data` and `${DATA_DIR}`.
- The validator does not contact Docker, pull images, resolve `env_file:` paths, read included
  files, or prove that an image tag exists in a registry.

## FAQ

<details>
<summary>How is this different from a YAML validator?</summary>

A YAML validator can tell you whether indentation and scalars parse. It cannot know that
`depends_on: [api]` points at a missing service, that `data:/var/lib/postgresql/data` needs a
top-level `volumes: data:` declaration, or that `70000:80` is not a valid published port. This tool
does those Compose-aware checks after the YAML parser succeeds.

</details>

<details>
<summary>Does the validator run Docker or upload my compose file?</summary>

No. It is a local static analysis pass compiled to WebAssembly. It does not run `docker compose
config`, pull images, resolve env files or contact registries. That keeps private image names,
internal hostnames and secret-looking environment values on your machine, but it also means backend
checks such as “does this image tag exist?” are intentionally out of scope.

</details>

<details>
<summary>Why are some findings warnings instead of errors?</summary>

Compose accepts patterns that are risky but not invalid: `image: nginx:latest`, a top-level
`version:` key, `privileged: true`, `network_mode: host`, or a service with both `build` and
`image`. The default report marks those as warnings so they are visible without claiming the file
cannot run. Turn on **Treat warnings as errors** if your CI policy wants them to fail the check.

</details>

<details>
<summary>What should I put in “Disable rules”?</summary>

Use stable rule ids separated by commas, spaces or newlines. For example, `image-tag` allows
floating tags in a throwaway development stack, and `quote-ports` suppresses the style warning for
unquoted short port syntax. Unknown ids are rejected so a typo cannot silently disable nothing.

</details>

<details>
<summary>Can I use the JSON output in CI?</summary>

Yes. Choose `json` for a machine-readable object with validity, counts and a `problems` array. Pair
it with `min_severity=error` for a blocking-only pre-commit check, or with `strict_warnings=true`
when warnings should count as failures in a pipeline.

</details>
