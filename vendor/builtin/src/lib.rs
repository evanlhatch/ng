#[allow(clippy::all)]
#[allow(warnings)]

// These types must match the structure used in the build.rs format! string
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] // <--- ADDED PartialEq, Eq, Hash
pub enum BuiltinKind {
    Function,
    Attrset,
    Const,
}

#[derive(Debug, Clone)]
pub struct Builtin {
    pub kind: BuiltinKind,
    pub is_global: bool,
    pub summary: &'static str,
    pub doc: Option<&'static str>,
    pub impure_only: bool,
    pub experimental_feature: Option<&'static str>,
}

// The build.rs writes the phf map literal to "generated.expr".
// We declare the static variable here and include the map literal as its value.
// The types `Builtin` and `BuiltinKind` above must be in scope (e.g. `crate::Builtin`).
// The format! in build.rs uses `crate::Builtin` and `crate::BuiltinKind`.
pub static ALL_BUILTINS: phf::Map<&'static str, Builtin> = 
    include!(concat!(env!("OUT_DIR"), "/generated.expr"));

pub fn init() {
    // Do nothing
}

// Make sure phf is in builtin/Cargo.toml dependencies:
// [dependencies]
// phf = { version = "0.11" } 
// The "macros" feature for phf is only needed if you use phf_map! or phf_set! macros directly in lib.rs.
// For just using a phf::Map, the base phf crate is enough.
