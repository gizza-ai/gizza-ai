## About this tool

**Docker Compose Generator** turns a short, paste-able spec — one line per service — into a
complete `docker-compose.yml`. Instead of clicking through a form for every port and volume, you
write what you already know:

```text
web: nginx:alpine ports=8080:80 depends=api
api: ghcr.io/acme/api:v1 ports=3000 env=PORT=3000
db: postgres:16-alpine volumes=dbdata:/var/lib/postgresql/data env=POSTGRES_PASSWORD=secret
```

Everything runs in your browser through WebAssembly, so image names, passwords and internal
hostnames never leave your machine.

The generator does the bookkeeping that hand-written compose files usually get wrong: named volumes
and networks referenced by any service are collected into the top-level `volumes:` and `networks:`
sections, `depends_on` targets are checked against the services you actually declared, and every
port mapping and environment value is emitted **quoted** so YAML cannot silently reinterpret it.

### Spec format

Each line is `name: image [key=value ...]`. Blank lines and lines starting with `#` are ignored.
Wrap any value containing a space or a comma in double quotes — `command="npm run start"`.

| Option | What it does |
| --- | --- |
| `image=` | Image reference. Also accepted bare, right after the colon. |
| `build=` | Build context path, instead of (or alongside) an image. |
| `ports=` | Comma-separated `8080:80`, `3000`, `127.0.0.1:5432:5432`, `8000-8005:8000-8005`, `53:53/udp`. |
| `expose=` | Container-only ports, not published to the host. |
| `volumes=` | Comma-separated `src:/container/path[:ro]`. A source that is not a path becomes a **named volume**. |
| `env=` | Comma-separated `KEY=value`. |
| `env_file=` | Comma-separated env file paths. |
| `depends=` | Other service names from the same spec. |
| `restart=` | `no`, `always`, `on-failure` or `unless-stopped`. |
| `command=` / `entrypoint=` | Override the image's command or entrypoint. |
| `container_name=`, `user=`, `working_dir=` | Fixed container name, run-as user, working directory. |
| `networks=` | Networks for this service, overriding the shared one. |
| `labels=` | Comma-separated `key=value` metadata. |
| `healthcheck=` | A shell command, emitted as a `CMD-SHELL` test with 30s interval, 10s timeout, 3 retries. |

### File-wide settings

- **Project name** — written as the top-level `name:`. Blank means Docker uses the directory name.
- **`version:` key** — omitted by default, which is what the current Compose specification wants.
  Pick `3.9`/`3.8`/`3.7`/`2.4` only if an old `docker-compose` v1 binary still demands one.
- **Shared network + driver** — one user-defined network attached to every service that does not set
  its own `networks=`, and declared at the top level with the chosen driver.
- **Default restart policy** — applied to every service without its own `restart=`.
- **Shared environment / env_file** — added to every service *beneath* its own entries, so a service
  that sets the same key wins.

### Worked example

Spec (above), project name `shop`, shared network `appnet`, default restart `unless-stopped`:

```yaml
name: shop
services:
  web:
    image: nginx:alpine
    restart: unless-stopped
    ports:
      - "8080:80"
    networks:
      - appnet
    depends_on:
      - api
  api:
    image: ghcr.io/acme/api:v1
    restart: unless-stopped
    ports:
      - "3000"
    environment:
      PORT: "3000"
    networks:
      - appnet
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_PASSWORD: "secret"
    volumes:
      - dbdata:/var/lib/postgresql/data
    networks:
      - appnet
volumes:
  dbdata:
networks:
  appnet:
    driver: bridge
```

Save it as `docker-compose.yml` and run `docker compose up -d`.

### Limits and edge cases

- **Maximum 25 services** per file. Past that the spec is rejected rather than truncated.
- A service needs an **image or a `build=` context**; a line with neither is an error.
- **Values containing spaces or commas must be double-quoted**, because the spec is split on
  whitespace and each option's list on commas.
- **Unknown options are errors, not warnings** — `portz=80` is rejected so a typo can never become a
  silently missing port mapping.
- The image must be the **first** token on the line; anywhere else, write it as `image=`.
- Long-tail Compose keys (`dns`, `ipc`, `mac_address`, `privileged`, `deploy.resources`, Swarm
  placement constraints, `configs`, `secrets`, per-service `profiles`) are **not** in the spec. The
  output is ordinary YAML — add them by hand afterwards.
- Healthcheck timings are fixed at `interval: 30s`, `timeout: 10s`, `retries: 3`,
  `start_period: 10s`; edit the generated block if your service needs different ones.

## FAQ

<details>
<summary>Why are my ports wrapped in quotes?</summary>

Because unquoted they are not strings. In YAML, `22:22` is a sexagesimal (base-60) number that
parses as `1342`, which is why hand-written compose files occasionally publish a port nobody asked
for. Every port mapping here is emitted double-quoted, so `- "22:22"` stays exactly what you wrote.
Environment values are quoted for the same reason: `DEBUG=true` becomes `DEBUG: "true"`, a string,
not a boolean.

</details>

<details>
<summary>Do I still need a `version:` key at the top?</summary>

No. The current Compose specification dropped it, and recent `docker compose` versions warn that
it is obsolete — so it is omitted by default. The dropdown is there only for the old standalone
`docker-compose` v1 binary, which refuses a file without one. If you are unsure, leave it on
"Omit".

</details>

<details>
<summary>When does a volume end up in the top-level `volumes:` section?</summary>

When its source is a **name** rather than a path. `dbdata:/var/lib/postgresql/data` mounts the named
volume `dbdata`, so `dbdata:` is declared at the top level — without that declaration Docker rejects
the file. `./site:/usr/share/nginx/html` is a bind mount of a host directory, so nothing is
declared. Sources starting with `/`, `./`, `../`, `~` or `$` are treated as paths.

</details>

<details>
<summary>How do I write a command or an environment value that contains spaces?</summary>

Put double quotes around it. The spec is split on whitespace, so `command=npm run dev` would read
`run` and `dev` as unknown options; `command="npm run dev"` keeps them together. The same applies
inside comma-separated lists — `env=GREETING="hello, world"` keeps the comma out of the list split.

</details>

<details>
<summary>Does `depends_on` wait for the service to be ready?</summary>

Not by itself. Plain `depends_on` only orders *startup* — the dependent container starts once the
other has been created, not once it is accepting connections. That is why the spec also has
`healthcheck=`: give the dependency a real readiness command (`healthcheck="pg_isready -U
postgres"`), then edit the generated `depends_on` list into the long form with
`condition: service_healthy` if you need Compose to actually wait.

</details>

<details>
<summary>What happens if a service depends on one that does not exist?</summary>

You get an error naming both services, and no YAML at all. The same is true for an invalid port, a
container path that is not absolute, a duplicate service name, an unknown restart policy, or a
service that depends on itself. The tool would rather refuse than hand you a file that
`docker compose up` rejects three minutes later.

</details>

<details>
<summary>Is the output stable enough to commit?</summary>

Yes. Services appear in spec order, and the derived `volumes:` and `networks:` sections are sorted,
so the same input always produces byte-identical output. That makes the generated file safe to check
in and re-generate — a diff only shows what you actually changed in the spec.

</details>
