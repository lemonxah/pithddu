use std::path::Path;

fn main() {
    let cfg = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("ui/app.slint", cfg).expect("compile ui/app.slint");

    gen_field_registry();
}

fn fmt_variant(f: &str) -> &'static str {
    match f {
        "int" => "Fmt::Int",
        "fixed1" => "Fmt::Fixed1",
        "fixed2" => "Fmt::Fixed2",
        "time" => "Fmt::Time",
        "sector" => "Fmt::Sector",
        "delta" => "Fmt::Delta",
        "string" => "Fmt::Str",
        other => panic!("unknown fmt token in field_registry.json: {other}"),
    }
}

fn gen_field_registry() {
    let json_path = Path::new("../firmware/main/field_registry.json");
    println!("cargo:rerun-if-changed={}", json_path.display());
    let raw = std::fs::read_to_string(json_path)
        .expect("read ../firmware/main/field_registry.json");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse field_registry.json");
    let fields = parsed["fields"].as_array().expect("fields array");

    let mut out = String::new();
    out.push_str(
        "// AUTO-GENERATED from firmware/main/field_registry.json by build.rs. Do not edit.\n",
    );

    out.push_str("pub const FIELD_NONE: usize = 0;\n");
    for (i, fd) in fields.iter().enumerate() {
        let name = fd["name"].as_str().unwrap().to_uppercase();
        out.push_str(&format!("pub const FIELD_{name}: usize = {};\n", i + 1));
    }
    out.push_str(&format!(
        "pub const FIELD_COUNT: usize = {};\n\n",
        fields.len() + 1
    ));

    out.push_str("pub static FIELD_REGISTRY: [FieldDef; FIELD_COUNT] = [\n");
    out.push_str("    FieldDef { name: \"\", fmt: Fmt::Int, scale: 1, label: \"\" },\n");
    for fd in fields {
        let name = fd["name"].as_str().unwrap();
        let fmt = fmt_variant(fd["fmt"].as_str().unwrap());
        let scale = fd["sc"].as_i64().unwrap_or(1);
        let label = fd["label"].as_str().unwrap();
        out.push_str(&format!(
            "    FieldDef {{ name: {name:?}, fmt: {fmt}, scale: {scale}, label: {label:?} }},\n"
        ));
    }
    out.push_str("];\n");

    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("field_registry_gen.rs");
    std::fs::write(&dest, out).expect("write field_registry_gen.rs");
}
