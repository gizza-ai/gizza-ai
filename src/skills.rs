//! Skill blocks embedded at compile time by build.rs.
//!
//! Each entry is `(block_name, wasm_bytes)`. initialize() iterates this list
//! and registers each as a WasmiBlock on the Wafer runtime.

include!(concat!(env!("OUT_DIR"), "/skills.rs"));
