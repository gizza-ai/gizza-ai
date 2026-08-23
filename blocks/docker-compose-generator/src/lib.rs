//! gizza-ai/docker-compose-generator — build a validated `docker-compose.yml`
//! from a short one-line-per-service spec. Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_docker_compose_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    services: String,
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    compose_version: String,
    #[serde(default)]
    network: String,
    #[serde(default)]
    network_driver: String,
    #[serde(default)]
    restart: String,
    #[serde(default)]
    env: String,
    #[serde(default)]
    env_file: String,
}

fn build(a: &Args) -> Result<String, String> {
    generate(
        &a.services,
        &a.project_name,
        &a.compose_version,
        &a.network,
        &a.network_driver,
        &a.restart,
        &a.env,
        &a.env_file,
    )
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("services")
                .required()
                .describe(
                    "The service spec, ONE SERVICE PER LINE as \"name: image [key=value ...]\", e.g. \"web: nginx:alpine ports=8080:80 depends=api\". Blank lines and lines starting with '#' are ignored; maximum 25 services. Per-service options: image=, build= (a build context instead of, or alongside, an image), ports= (comma-separated, e.g. \"8080:80,127.0.0.1:5432:5432/tcp\"), expose=, volumes= (comma-separated \"src:/container/path[:ro]\"; a source that is not a path becomes a named volume declared at the top level), env= (comma-separated KEY=value), env_file=, depends= (other service names), restart= (no/always/on-failure/unless-stopped), command=, entrypoint=, container_name=, user=, working_dir=, networks=, labels= (comma-separated key=value) and healthcheck= (a shell command run as CMD-SHELL). Wrap any value containing a space or comma in double quotes, e.g. command=\"npm run start\".",
                ),
        )
        .param(
            Param::string("project_name")
                .describe("Top-level Compose project name written as 'name:', e.g. \"shop\". Omitted from the file when blank, in which case Docker uses the directory name."),
        )
        .param(
            Param::enumv("compose_version", ["none", "3.9", "3.8", "3.7", "2.4"])
                .default("none")
                .describe("Legacy 'version:' key. Default 'none' omits it, which is what the current Compose specification wants; pick 3.9/3.8/3.7/2.4 only for an old docker-compose v1 binary that still requires it."),
        )
        .param(
            Param::string("network")
                .describe("Name of one shared user-defined network attached to every service that does not set its own networks=, e.g. \"appnet\". It is also declared in the top-level 'networks:' section. Blank leaves services on Compose's implicit default network."),
        )
        .param(
            Param::enumv("network_driver", ["bridge", "host", "overlay", "macvlan", "ipvlan"])
                .default("bridge")
                .describe("Driver for the shared network named by 'network': 'bridge' (default, single host), 'host', 'overlay' (Swarm), 'macvlan' or 'ipvlan'. Ignored when 'network' is blank."),
        )
        .param(
            Param::enumv("restart", ["none", "no", "always", "on-failure", "unless-stopped"])
                .default("none")
                .describe("Default restart policy for every service that does not set its own restart=. Default 'none' emits no restart key at all; 'no' emits the explicit Docker policy \"no\"."),
        )
        .param(
            Param::string("env")
                .describe("File-wide environment as KEY=value lines, one per line, e.g. \"TZ=UTC\\nLOG_LEVEL=info\". Added to every service beneath its own env=, which wins on a clash. Blank and '#' comment lines are skipped and a leading 'export ' is dropped."),
        )
        .param(
            Param::string("env_file")
                .describe("File-wide env_file paths added to every service, one per line or comma-separated, e.g. \".env\". A service's own env_file= entries are appended after these."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docker-compose-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a docker-compose.yml from a short one-line-per-service spec",
    skill(
        description = "Generate a complete, validated docker-compose.yml from a short spec with one line per service: \"name: image [key=value ...]\", for example \"web: nginx:alpine ports=8080:80 depends=api\". Per-service options cover image, build context, ports, expose, volumes, environment, env_file, depends_on, restart policy, command, entrypoint, container_name, user, working_dir, networks, labels and a CMD-SHELL healthcheck; values with spaces or commas go in double quotes. File-wide settings add a project name, the legacy version key, a shared user-defined network and driver, a default restart policy, and shared environment/env_file entries that each service can override. Named volumes and networks referenced by the services are collected into the top-level volumes: and networks: sections automatically, ports and environment values are quoted so YAML cannot reinterpret them as numbers or booleans, and every service name, port, volume, dependency and policy is validated before any YAML is emitted. Maximum 25 services; the same input always produces the same bytes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "docker-compose-generator", |a: Args| {
            build(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_schema() {
        println!("SCHEMA_START{}SCHEMA_END", schema_json());
    }

    #[test]
    fn build_wires_every_arg_through_to_the_core() {
        let a = Args {
            services: "web: nginx:alpine ports=8080:80\ndb: postgres:16-alpine volumes=dbdata:/var/lib/postgresql/data".to_string(),
            project_name: "shop".to_string(),
            compose_version: "none".to_string(),
            network: "appnet".to_string(),
            network_driver: "bridge".to_string(),
            restart: "unless-stopped".to_string(),
            env: "TZ=UTC".to_string(),
            env_file: ".env".to_string(),
        };
        let out = build(&a).unwrap();
        assert!(out.starts_with("name: shop\nservices:\n"), "{out}");
        assert!(out.contains("      - \"8080:80\"\n"), "{out}");
        assert!(out.contains("    restart: unless-stopped\n"), "{out}");
        assert!(out.contains("      TZ: \"UTC\"\n"), "{out}");
        assert!(out.contains("      - .env\n"), "{out}");
        assert!(out.contains("volumes:\n  dbdata:\n"), "{out}");
        assert!(
            out.contains("networks:\n  appnet:\n    driver: bridge\n"),
            "{out}"
        );
    }

    #[test]
    fn build_surfaces_core_errors() {
        let a = Args {
            services: "web nginx".to_string(),
            project_name: String::new(),
            compose_version: "none".to_string(),
            network: String::new(),
            network_driver: "bridge".to_string(),
            restart: "none".to_string(),
            env: String::new(),
            env_file: String::new(),
        };
        assert!(build(&a).unwrap_err().contains("expected"));
    }
}
