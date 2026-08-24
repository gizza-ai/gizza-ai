//! gizza-ai/k8s-manifest-splitter — chat skill block on the shared tool abstraction.
//! Splits a multi-document Kubernetes YAML stream into individual resources, names
//! each one from a filename template, and renders a file bundle, an index table,
//! JSON, a kustomization.yaml or a shell script. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_k8s_manifest_splitter_core::{split, Options, Output, Sort, DEFAULT_TEMPLATE};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    manifest: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_template")]
    filename_template: String,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    skip_non_k8s: bool,
    #[serde(default = "default_true")]
    expand_lists: bool,
    #[serde(default)]
    include_triple_dash: bool,
}

fn default_output() -> String {
    "files".into()
}
fn default_template() -> String {
    DEFAULT_TEMPLATE.into()
}
fn default_sort() -> String {
    "document".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("manifest")
                .required()
                .describe("The multi-document Kubernetes YAML to split, e.g. the output of 'helm template', 'kustomize build' or 'kubectl get -o yaml'. Documents are separated by a '---' line. Up to 2,000,000 bytes and 1000 documents."),
        )
        .param(
            Param::enumv("output", ["files", "index", "json", "kustomization", "shell"])
                .default("files")
                .describe("What to render: 'files' (default) every document under a '# ===== <filename> =====' header; 'index' an aligned table of kind/name/namespace/apiVersion/lines/filename; 'json' one object per resource including its YAML; 'kustomization' a kustomization.yaml listing the filenames; 'shell' a POSIX sh script that writes every file."),
        )
        .param(
            Param::string("filename_template")
                .default("{kind}-{name}.yaml")
                .describe("Filename pattern for each resource. Placeholders: {kind} (lowercased), {Kind}, {name}, {namespace} ('default' when unset), {apiVersion}, {group} ('core' for the core group), {version}, {index} (1-based, zero-padded), plus any dotted field path read straight from the document, e.g. {metadata.labels.app} or {spec.replicas}. A '/' makes directories, e.g. '{kind}/{name}.yaml'. Default '{kind}-{name}.yaml'."),
        )
        .param(
            Param::string("include")
                .default("")
                .describe("Keep only resources matching these comma-separated selectors. A selector is 'Kind' or 'Kind/name', case-insensitive, with * and ? wildcards, e.g. 'Deployment,Service/web-*'. Blank (default) keeps everything."),
        )
        .param(
            Param::string("exclude")
                .default("")
                .describe("Drop resources matching these comma-separated 'Kind' or 'Kind/name' selectors (same wildcard syntax as include; applied after include). Blank (default) drops nothing."),
        )
        .param(
            Param::enumv("sort", ["document", "kind", "name", "apply"])
                .default("document")
                .describe("Output order: 'document' (default) keeps the input order; 'kind' sorts alphabetically by kind then name; 'name' by name then kind; 'apply' uses the dependency-safe install order (Namespace, ServiceAccount, Secret, ConfigMap, CRD, RBAC, Service, workloads, Ingress)."),
        )
        .param(
            Param::boolean("skip_non_k8s")
                .default(false)
                .describe("Drop documents that are missing apiVersion, kind or metadata.name instead of naming them 'unknown-unnamed.yaml'. Default false."),
        )
        .param(
            Param::boolean("expand_lists")
                .default(true)
                .describe("Split a 'kind: List' / '*List' wrapper (what 'kubectl get -o yaml' returns) into its items, one resource per item. Expanded items are re-serialized, so their comments are lost; every other document is kept byte-verbatim. Default true."),
        )
        .param(
            Param::boolean("include_triple_dash")
                .default(false)
                .describe("Start each document in the 'files' output with a '---' line, so the bundle stays a valid multi-document YAML stream. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        output: Output::parse(&a.output)?,
        filename_template: a.filename_template.clone(),
        include: a.include.clone(),
        exclude: a.exclude.clone(),
        sort: Sort::parse(&a.sort)?,
        skip_non_k8s: a.skip_non_k8s,
        expand_lists: a.expand_lists,
        include_triple_dash: a.include_triple_dash,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/k8s-manifest-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a multi-document Kubernetes YAML into individual resource files",
    skill(
        description = "Split a multi-document Kubernetes YAML stream (helm template / kustomize build / kubectl get -o yaml output) into its individual resources, indexed by kind and name. Each resource gets a filename from a template (default '<kind>-<name>.yaml', with {namespace}/{group}/{index} placeholders and '/' for directories). Render the split as a file bundle, an index table, JSON, a kustomization.yaml, or a shell script that writes the files. Filter with Kind/name wildcard selectors, sort by kind, name or dependency-safe apply order, expand 'kind: List' wrappers, and drop non-Kubernetes documents. Document bodies stay byte-verbatim, so comments survive. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "k8s-manifest-splitter", |a: Args| {
            let opts = options(&a).map_err(SkillError::InvalidArgs)?;
            split(&a.manifest, &opts).map_err(SkillError::InvalidArgs)
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "manifest": { "type": "string", "description": "The multi-document Kubernetes YAML to split, e.g. the output of 'helm template', 'kustomize build' or 'kubectl get -o yaml'. Documents are separated by a '---' line. Up to 2,000,000 bytes and 1000 documents." },
                    "output": { "type": "string", "enum": ["files", "index", "json", "kustomization", "shell"], "default": "files", "description": "What to render: 'files' (default) every document under a '# ===== <filename> =====' header; 'index' an aligned table of kind/name/namespace/apiVersion/lines/filename; 'json' one object per resource including its YAML; 'kustomization' a kustomization.yaml listing the filenames; 'shell' a POSIX sh script that writes every file." },
                    "filename_template": { "type": "string", "default": "{kind}-{name}.yaml", "description": "Filename pattern for each resource. Placeholders: {kind} (lowercased), {Kind}, {name}, {namespace} ('default' when unset), {apiVersion}, {group} ('core' for the core group), {version}, {index} (1-based, zero-padded), plus any dotted field path read straight from the document, e.g. {metadata.labels.app} or {spec.replicas}. A '/' makes directories, e.g. '{kind}/{name}.yaml'. Default '{kind}-{name}.yaml'." },
                    "include": { "type": "string", "default": "", "description": "Keep only resources matching these comma-separated selectors. A selector is 'Kind' or 'Kind/name', case-insensitive, with * and ? wildcards, e.g. 'Deployment,Service/web-*'. Blank (default) keeps everything." },
                    "exclude": { "type": "string", "default": "", "description": "Drop resources matching these comma-separated 'Kind' or 'Kind/name' selectors (same wildcard syntax as include; applied after include). Blank (default) drops nothing." },
                    "sort": { "type": "string", "enum": ["document", "kind", "name", "apply"], "default": "document", "description": "Output order: 'document' (default) keeps the input order; 'kind' sorts alphabetically by kind then name; 'name' by name then kind; 'apply' uses the dependency-safe install order (Namespace, ServiceAccount, Secret, ConfigMap, CRD, RBAC, Service, workloads, Ingress)." },
                    "skip_non_k8s": { "type": "boolean", "default": false, "description": "Drop documents that are missing apiVersion, kind or metadata.name instead of naming them 'unknown-unnamed.yaml'. Default false." },
                    "expand_lists": { "type": "boolean", "default": true, "description": "Split a 'kind: List' / '*List' wrapper (what 'kubectl get -o yaml' returns) into its items, one resource per item. Expanded items are re-serialized, so their comments are lost; every other document is kept byte-verbatim. Default true." },
                    "include_triple_dash": { "type": "boolean", "default": false, "description": "Start each document in the 'files' output with a '---' line, so the bundle stays a valid multi-document YAML stream. Default false." }
                },
                "required": ["manifest"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn options_reject_an_unknown_choice() {
        let args = Args {
            manifest: String::new(),
            output: "yaml".into(),
            filename_template: default_template(),
            include: String::new(),
            exclude: String::new(),
            sort: default_sort(),
            skip_non_k8s: false,
            expand_lists: true,
            include_triple_dash: false,
        };
        assert!(options(&args).unwrap_err().contains("unknown output 'yaml'"));
    }
}
