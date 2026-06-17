//! gizza-cli — run gizza skill tools headlessly.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod render;
pub mod runtime;

mod skill_wasms {
    include!(concat!(env!("OUT_DIR"), "/skill_wasms.rs"));
}
pub(crate) use skill_wasms::SKILL_WASMS;
