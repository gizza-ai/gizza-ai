//! gizza-ai/obj-to-gltf — chat skill block on the shared tool abstraction.
//! Converts pasted Wavefront OBJ text (and optional pasted MTL text) into a
//! self-contained glTF JSON asset or GLB data URL. Pure compute: no host I/O,
//! no network, and no texture-file embedding.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    obj: String,
    #[serde(default)]
    mtl: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_up_axis")]
    up_axis: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_normals")]
    normals: String,
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    unlit: bool,
    #[serde(default)]
    double_sided: bool,
}

fn default_to() -> String {
    "gltf".to_string()
}
fn default_up_axis() -> String {
    "y".to_string()
}
fn default_scale() -> f64 {
    1.0
}
fn default_normals() -> String {
    "auto".to_string()
}
fn default_name() -> String {
    "model".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("obj").required().describe(
                "Wavefront OBJ text to convert. Paste the OBJ contents, including v/vt/vn/f/usemtl lines. Faces may be triangles, quads, or n-gons; polygons are fan-triangulated. Relative negative indices are supported. Texture image files referenced by map_* lines cannot be embedded here.",
            ),
        )
        .param(
            Param::string("mtl").default("").describe(
                "Optional Wavefront MTL text pasted alongside the OBJ. Supports newmtl, Kd, d/Tr, Ke, Ks, Ns and texture-reference detection. Texture file contents are not accepted; map_* references are ignored after materials are colored.",
            ),
        )
        .param(
            Param::enumv("to", ["gltf", "glb"]).default("gltf").describe(
                "Output container. gltf returns pretty-printed glTF 2.0 JSON with its binary buffer embedded as a data URI. glb returns a data:model/gltf-binary;base64 URL that can be saved as a .glb file. Default gltf.",
            ),
        )
        .param(
            Param::enumv("up_axis", ["y", "z"]).default("y").describe(
                "Which axis is up in the source OBJ. y leaves coordinates in glTF's native Y-up frame. z rotates Z-up CAD/3D-print models into glTF's Y-up frame. Default y.",
            ),
        )
        .param(
            Param::number("scale").default(1.0).describe(
                "Uniform scale factor applied to every vertex before export. Use 0.001 for millimetres to metres or 100 for metres to centimetres. Must be finite and non-zero. Default 1.",
            ),
        )
        .param(
            Param::enumv("normals", ["auto", "flat", "none"]).default("auto").describe(
                "Normal generation. auto uses OBJ vn normals when present and computes flat face normals for missing faces. flat ignores vn and recomputes flat normals. none omits NORMAL attributes. Default auto.",
            ),
        )
        .param(
            Param::string("name").default("model").describe(
                "Scene, node and mesh name written into the glTF asset. Blank falls back to model. Default model.",
            ),
        )
        .param(
            Param::boolean("unlit").default(false).describe(
                "When true, marks every material with KHR_materials_unlit and adds the extension to extensionsUsed. Useful for CAD or icon-like assets where colors should not be shaded. Default false.",
            ),
        )
        .param(
            Param::boolean("double_sided").default(false).describe(
                "When true, sets doubleSided on emitted materials so viewers render both front and back faces. Useful for single-plane or thin-sheet OBJ meshes. Default false.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn convert_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_obj_to_gltf_core::run(
        &a.obj,
        &a.mtl,
        &a.to,
        &a.up_axis,
        a.scale,
        &a.normals,
        &a.name,
        a.unlit,
        a.double_sided,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/obj-to-gltf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert pasted Wavefront OBJ and optional MTL text into self-contained glTF or GLB.",
    skill(
        description = "Convert pasted Wavefront OBJ model text, plus optional pasted MTL material text, into a self-contained glTF 2.0 JSON asset or GLB data URL. Handles v/vt/vn/f/usemtl, relative OBJ indices, fan-triangulates quads and n-gons, groups primitives by material, embeds the binary buffer in glTF JSON, and can emit GLB as data:model/gltf-binary;base64. Options include output format (gltf/glb), source up axis (y/z), scale, normals mode (auto/flat/none), scene name, KHR_materials_unlit, and double-sided materials. Texture image files, Draco/meshopt compression, scene hierarchy, and batch/multi-file upload are out of scope for this single pasted-text tool. Runs offline with no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "obj-to-gltf", convert_args) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_schema_exposes_all_cli_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "obj",
            "mtl",
            "to",
            "up_axis",
            "scale",
            "normals",
            "name",
            "unlit",
            "double_sided",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "{key} has a weak description"
            );
        }
        assert_eq!(schema["required"], serde_json::json!(["obj"]));
        assert_eq!(props["to"]["enum"], serde_json::json!(["gltf", "glb"]));
        assert_eq!(props["up_axis"]["enum"], serde_json::json!(["y", "z"]));
        assert_eq!(
            props["normals"]["enum"],
            serde_json::json!(["auto", "flat", "none"])
        );
        assert_eq!(props["scale"]["default"], serde_json::json!(1.0));
        assert_eq!(props["double_sided"]["default"], serde_json::json!(false));
    }

    #[test]
    fn serde_defaults_match_schema_defaults() {
        let a: Args =
            serde_json::from_str(r#"{"obj":"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3"}"#).unwrap();
        assert_eq!(a.to, "gltf");
        assert_eq!(a.up_axis, "y");
        assert_eq!(a.scale, 1.0);
        assert_eq!(a.normals, "auto");
        assert_eq!(a.name, "model");
        assert!(!a.unlit);
        assert!(!a.double_sided);
        let out = convert_args(a).unwrap();
        assert!(out.contains("\"version\":"));
        assert!(out.contains("\"2.0\""));
        assert!(out.contains("\"meshes\":"));
    }

    #[test]
    fn bad_output_format_is_an_invalid_argument() {
        let a: Args =
            serde_json::from_str(r#"{"obj":"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3","to":"fbx"}"#)
                .unwrap();
        let err = convert_args(a).unwrap_err();
        match err {
            SkillError::InvalidArgs(msg) => {
                assert!(msg.contains("expected 'gltf' or 'glb'"), "{msg}")
            }
            other => panic!("wrong error: {other:?}"),
        }
    }
}
