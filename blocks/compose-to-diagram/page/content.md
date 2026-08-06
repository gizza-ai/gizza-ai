## About this tool

Convert a `docker-compose.yml` or `compose.yaml` file into Mermaid flowchart text.
The parser reads the parts that describe architecture — services, `image` or
`build`, `depends_on`, `links`, `network_mode: service:...`, ports, volumes,
networks, profiles, restart policy, and replica counts — and ignores runtime
details such as commands and environment values.

Use raw Mermaid output for Mermaid Live, GitHub Markdown, Notion, and IDE preview
plugins. Use Markdown output when you want a ready-to-paste fenced code block.
Use Summary output for a text audit of services, dependencies, duplicate host
ports, unused declarations, and circular dependency chains.

Worked example:

1. Paste a Compose file with `web`, `api`, and `db` services.
2. Set direction to `LR` for a left-to-right architecture view.
3. Leave networks as subgraphs and keep ports/volumes enabled.
4. Copy the Mermaid flowchart into your README or diagram previewer.

Limits and edge cases: this tool accepts YAML up to 2 MB and up to 500 services.
It does not run Docker, pull images, read `.env`, resolve includes, or inspect
external compose files referenced by `extends.file`; those references are reported
as warnings in the summary.

## FAQ

<details>
<summary>Does this execute my Compose file?</summary>

No. The tool only parses YAML text and renders a diagram. It does not contact
Docker, fetch images, interpolate `.env`, or run any service commands.

</details>

<details>
<summary>Which dependency forms are shown?</summary>

Both `depends_on: [db]` and the long map form with `condition:` are supported.
The tool also draws `links`, `network_mode: service:name`, and same-file
`extends` relationships so hidden service coupling is visible.

</details>

<details>
<summary>How should I show shared networks?</summary>

Use `subgraph` for the common case where each service mostly belongs to one
network. Use `node` when services join many networks and you want every network
drawn as a separate node. Use `off` for a dependency-only diagram.

</details>

<details>
<summary>Can I use the output directly in a README?</summary>

Yes. Choose `markdown` output for a ready-to-paste fenced `mermaid` block, or
choose raw `mermaid` output if your documentation system already provides the
fence.

</details>
