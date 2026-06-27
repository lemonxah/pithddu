use super::format::Fmt;

pub struct FieldDef {
    pub name: &'static str,
    pub fmt: Fmt,
    pub scale: i32,
    #[allow(dead_code)]
    pub label: &'static str,
}

#[allow(dead_code)]
mod gen {
    use super::{FieldDef, Fmt};
    include!(concat!(env!("OUT_DIR"), "/field_registry_gen.rs"));
}
pub use gen::*;

pub fn field_id_from_str(s: &str) -> usize {
    for i in 1..FIELD_COUNT {
        if FIELD_REGISTRY[i].name == s {
            return i;
        }
    }
    FIELD_NONE
}
