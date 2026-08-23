# docker-compose-generator — competitor analysis (2026-08-23)

Scan run BEFORE implementation, per the create-next-tool recipe. All notes are paraphrased
observations of publicly visible tool surfaces; no competitor copy, branding, or trademarks were
copied into this repo.

## Competitors skimmed

1. **GenTools.io — Docker Compose Generator** (`gentools.io/docker-compose-generator`) — visual
   click-to-add builder with a catalogue of ~20 pre-configured service templates (Postgres, MySQL,
   MongoDB, Redis, MariaDB, Elasticsearch, RabbitMQ, Kafka, Zookeeper, Nginx, Traefik, Caddy,
   Prometheus, Grafana, Jaeger, Adminer, MailHog, MinIO, Portainer, Node/Python/PHP runtimes) and six
   "quick stack" bundles (LAMP, MEAN, WordPress, monitoring, message queue, dev environment).
2. **EaseCloud — Docker Compose Generator**
   (`easecloud.io/tools/code-generators/docker-compose-generator/`) — simple repeated-row form:
   service name, image, ports, environment, volumes, with an "Add Service" button, a compose-version
   select, and include-networks / include-volumes checkboxes. Notably omits restart, depends_on,
   container_name, command, healthcheck and build.
3. **8gwifi.org — Docker Compose Generator** (`8gwifi.org/dc.jsp`) — the widest per-service field
   set of the three: service name, image, container_name, entrypoint, volumes, environment, labels,
   port mappings, expose, dns, dns_search, user, working_dir, hostname, domainname, ipc, mac_address,
   privileged, restart policy, links, depends_on, cpu/memory limits and reservations, Swarm placement
   constraints, healthcheck. Emits `version` + `services` + optional `volumes`/`networks`.

## Table stakes → decision

| Capability | Seen at | In model? | Where it landed |
|---|---|---|---|
| Per-service `image` | all 3 | yes | spec DSL: positional after `name:`, or `image=` |
| Per-service `ports` | all 3 | yes | `ports=` key; emitted **quoted** (the base-60 trap) |
| Per-service `volumes` | all 3 | yes | `volumes=` key; named volumes auto-declared top-level |
| Per-service `environment` | all 3 | yes | `env=` key + global `env` applied to every service |
| Top-level `services` / `volumes` / `networks` | all 3 | yes | emitted; `volumes:` derived, `networks:` from `network` |
| Compose `version` key select | GenTools, EaseCloud | yes | `compose_version` enum, default `none` (modern spec drops it) |
| Named network + driver | GenTools, EaseCloud | yes | `network` + `network_driver` params |
| `restart` policy | 8gwifi | yes | global `restart` default + per-service `restart=` override |
| `depends_on` | GenTools, 8gwifi | yes | `depends=` key, validated against declared services |
| `container_name` | 8gwifi | yes | `container_name=` key |
| `command` / `entrypoint` | 8gwifi | yes | `command=` / `entrypoint=` keys (quoted values allowed) |
| `healthcheck` | GenTools, 8gwifi | yes | `healthcheck=` key → `CMD-SHELL` test + interval/timeout/retries |
| `expose` | 8gwifi | yes | `expose=` key |
| `user`, `working_dir` | 8gwifi | yes | `user=` / `working_dir=` keys |
| Per-service `labels` | 8gwifi | yes | `labels=` key |
| `build` context (image alternative) | GenTools | yes | `build=` key |
| `env_file` | — (common in real compose files) | yes | global `env_file` param + per-service `env_file=` |
| Project name (`name:` top-level) | — (modern Compose spec) | yes | `project_name` param |
| Preset / quick-stack bundles | GenTools, Tsveker | yes | shipped as `[[example]]` chips (LAMP-ish, MEAN-ish, WordPress, monitoring) |
| Copy / download output | all 3 | yes | generator gives Copy + Download free on `format = "text"` pages |

### Out of model (listed, not built)

- **A curated image catalogue with click-to-add rows.** Our surface is one text spec, not a dynamic
  form; a 20-entry image picker is site-side UI, not a block parameter. Mitigated by preset chips
  that prefill complete multi-service specs.
- **Auto-derived per-image healthchecks** (GenTools infers `pg_isready` for Postgres etc.). That is a
  hardcoded knowledge base of third-party images; we take an explicit `healthcheck=` command instead
  so the output stays honest and version-independent.
- **Swarm placement constraints, dns/dns_search, ipc, mac_address, privileged, cpu/memory
  reservations** (8gwifi). Long-tail keys, each an escape hatch; the generated YAML is meant to be
  edited, and adding a dozen rarely-used keys to the DSL costs more than it returns. Documented as a
  limit on the page rather than silently dropped.
- **Reverse-generation from running containers** (ComposeIt, PyPI). Requires a Docker daemon; there
  is no daemon in a wasm sandbox.

## UX control patterns matched

- Competitors ship **preset stacks** → we ship four `[[example]]` chips that prefill a full spec.
- Competitors ship a **compose-version dropdown** → `Param::enumv` renders a real `<select>`.
- The spec field is `multiline = true` so pasted multi-line specs keep their newlines.
- Copy result + Reset + Download come from the generic text-page driver.

## Differentiator

The three competitors all require clicking through a form. The backlog row asks for "a short spec",
and that is the actual advantage: one paste-able, diff-able, version-controllable line per service
that also works verbatim from the CLI and from chat, where a click-to-add form cannot go.
