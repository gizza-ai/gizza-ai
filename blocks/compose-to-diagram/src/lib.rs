//! gizza-ai/compose-to-diagram — turn a docker-compose.yml into a Mermaid
//! architecture diagram, as a chat skill block on the shared tool abstraction.
//!
//! Thin wrapper around `gizza-ai-compose-to-diagram-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    compose: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    networks: String,
    #[serde(default = "default_true")]
    ports: bool,
    #[serde(default = "default_true")]
    volumes: bool,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    profile: String,
    #[serde(default = "default_true")]
    styled: bool,
    #[serde(default)]
    title: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("compose")
                .required()
                .describe("The docker-compose.yml (or compose.yaml) contents to diagram. Must be a YAML mapping with a top-level 'services:' key; 'networks:' and 'volumes:' blocks are read when present. Understood per service: image, build (string or context/dockerfile map), depends_on (list form or map form with condition:), links, network_mode: service:NAME, extends, ports (short '8080:80' / '127.0.0.1:8080:80/udp' and long target/published/protocol form), volumes (short 'name:/path:ro' and long type/source/target form), networks (list or map), profiles, restart and deploy.replicas. Environment variables, commands and healthcheck bodies are ignored, ${VAR} placeholders are left verbatim (no .env file is read), and nothing is executed or fetched. Maximum 2000000 bytes and 500 services."),
        )
        .param(
            Param::enumv("direction", ["TD", "LR", "BT", "RL"])
                .default("TD")
                .describe("Flowchart orientation: 'TD' top-down (default), 'LR' left-to-right (usually best for long dependency chains), 'BT' bottom-up, 'RL' right-to-left."),
        )
        .param(
            Param::enumv("networks", ["subgraph", "node", "off"])
                .default("subgraph")
                .describe("How networks are drawn. 'subgraph' (default) boxes each service inside its FIRST declared network and draws a dotted edge for any additional network it joins. 'node' draws every network as a separate hexagon node with dotted membership edges — clearer when services span many networks. 'off' hides networks entirely."),
        )
        .param(
            Param::boolean("ports")
                .default(true)
                .describe("Draw published ports as their own nodes pointing into the service (default true). Labels use the compose mapping, e.g. '8080:80', '127.0.0.1:8080:80/udp', or 'expose 5432' for a container-only port."),
        )
        .param(
            Param::boolean("volumes")
                .default(true)
                .describe("Draw named volumes (cylinders) and bind mounts (slanted boxes) as nodes, with the container mount path on the edge and '(ro)' for read-only mounts (default true). Anonymous volumes — a bare container path with no source — are skipped."),
        )
        .param(
            Param::enumv("labels", ["name", "image", "full"])
                .default("image")
                .describe("How much detail goes inside each service node. 'name' is the service name only, 'image' (default) adds the image tag or 'build: <context>', and 'full' also adds replica count, restart policy and profiles."),
        )
        .param(
            Param::string("profile")
                .default("")
                .describe("Show only the services active in this compose profile, plus services that declare no profiles at all (Docker's own rule). Blank (default) shows every service regardless of profile. Fails if no service matches."),
        )
        .param(
            Param::boolean("styled")
                .default(true)
                .describe("Emit Mermaid classDef colour classes so services, ports, volumes, bind mounts, networks and undefined references each render in a distinct colour (default true). Turn off for plain uncoloured Mermaid that inherits your site or editor theme."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional diagram title. In Mermaid output it becomes a '---\\ntitle: …\\n---' front-matter block; in Markdown output it becomes an H1 above the fenced diagram; in the summary it is appended to the heading. Blank (default) omits it."),
        )
        .param(
            Param::enumv("output", ["mermaid", "markdown", "summary"])
                .default("mermaid")
                .describe("What to return. 'mermaid' (default) is raw flowchart text to paste into mermaid.live, a README, GitHub, Notion or an IDE preview. 'markdown' wraps that in a ```mermaid fenced code block. 'summary' is a plain-text audit of the file instead of a diagram: every service with its image, ports, volumes, networks and dependencies, plus warnings for undefined depends_on targets, duplicate host ports, unused network/volume declarations and circular dependency chains."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ComposeToDiagram;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/compose-to-diagram",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a docker-compose.yml into a Mermaid diagram of services, dependencies, ports, volumes and networks.",
    skill(
        description = "Turn a docker-compose.yml into a Mermaid flowchart of the stack's architecture. Reads services and their image/build, depends_on edges (both the short list form and the long map form with condition: service_healthy), links, network_mode: service:NAME, same-file extends, published ports, named volumes and bind mounts, network membership, profiles, restart policy and replica counts — then renders a flowchart with networks as subgraphs or nodes, ports and volumes as their own shapes, and colour classes per element type. 'direction' sets orientation, 'labels' controls node detail, 'profile' filters to one compose profile, and 'output' switches between raw Mermaid, a fenced Markdown block, and a plain-text summary that also reports undefined depends_on targets, duplicate host ports, unused declarations and circular dependency chains. Pure text analysis: nothing is executed, pulled or fetched.",
        parameters = schema_json()
    ),
)]
impl ComposeToDiagram {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "compose-to-diagram", |a: Args| {
            gizza_ai_compose_to_diagram_core::compose_to_diagram(
                &a.compose,
                &a.direction,
                &a.networks,
                a.ports,
                a.volumes,
                &a.labels,
                &a.profile,
                a.styled,
                &a.title,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "compose": { "type": "string", "description": "The docker-compose.yml (or compose.yaml) contents to diagram. Must be a YAML mapping with a top-level 'services:' key; 'networks:' and 'volumes:' blocks are read when present. Understood per service: image, build (string or context/dockerfile map), depends_on (list form or map form with condition:), links, network_mode: service:NAME, extends, ports (short '8080:80' / '127.0.0.1:8080:80/udp' and long target/published/protocol form), volumes (short 'name:/path:ro' and long type/source/target form), networks (list or map), profiles, restart and deploy.replicas. Environment variables, commands and healthcheck bodies are ignored, ${VAR} placeholders are left verbatim (no .env file is read), and nothing is executed or fetched. Maximum 2000000 bytes and 500 services." },
                    "direction": { "type": "string", "enum": ["TD", "LR", "BT", "RL"], "default": "TD", "description": "Flowchart orientation: 'TD' top-down (default), 'LR' left-to-right (usually best for long dependency chains), 'BT' bottom-up, 'RL' right-to-left." },
                    "networks": { "type": "string", "enum": ["subgraph", "node", "off"], "default": "subgraph", "description": "How networks are drawn. 'subgraph' (default) boxes each service inside its FIRST declared network and draws a dotted edge for any additional network it joins. 'node' draws every network as a separate hexagon node with dotted membership edges — clearer when services span many networks. 'off' hides networks entirely." },
                    "ports": { "type": "boolean", "default": true, "description": "Draw published ports as their own nodes pointing into the service (default true). Labels use the compose mapping, e.g. '8080:80', '127.0.0.1:8080:80/udp', or 'expose 5432' for a container-only port." },
                    "volumes": { "type": "boolean", "default": true, "description": "Draw named volumes (cylinders) and bind mounts (slanted boxes) as nodes, with the container mount path on the edge and '(ro)' for read-only mounts (default true). Anonymous volumes — a bare container path with no source — are skipped." },
                    "labels": { "type": "string", "enum": ["name", "image", "full"], "default": "image", "description": "How much detail goes inside each service node. 'name' is the service name only, 'image' (default) adds the image tag or 'build: <context>', and 'full' also adds replica count, restart policy and profiles." },
                    "profile": { "type": "string", "default": "", "description": "Show only the services active in this compose profile, plus services that declare no profiles at all (Docker's own rule). Blank (default) shows every service regardless of profile. Fails if no service matches." },
                    "styled": { "type": "boolean", "default": true, "description": "Emit Mermaid classDef colour classes so services, ports, volumes, bind mounts, networks and undefined references each render in a distinct colour (default true). Turn off for plain uncoloured Mermaid that inherits your site or editor theme." },
                    "title": { "type": "string", "default": "", "description": "Optional diagram title. In Mermaid output it becomes a '---\\ntitle: …\\n---' front-matter block; in Markdown output it becomes an H1 above the fenced diagram; in the summary it is appended to the heading. Blank (default) omits it." },
                    "output": { "type": "string", "enum": ["mermaid", "markdown", "summary"], "default": "mermaid", "description": "What to return. 'mermaid' (default) is raw flowchart text to paste into mermaid.live, a README, GitHub, Notion or an IDE preview. 'markdown' wraps that in a ```mermaid fenced code block. 'summary' is a plain-text audit of the file instead of a diagram: every service with its image, ports, volumes, networks and dependencies, plus warnings for undefined depends_on targets, duplicate host ports, unused network/volume declarations and circular dependency chains." }
                },
                "required": ["compose"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
