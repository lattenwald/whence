use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let languages = manifest.parent().unwrap().join("languages");
    println!("cargo:rerun-if-changed={}", languages.display());

    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&languages)
        .unwrap_or_else(|e| panic!("read {}: {e}", languages.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut table = String::from(
        "pub fn table() -> Vec<(&'static str, tree_sitter::Language, &'static str, &'static str)> {\n    vec![\n",
    );
    for dir in dirs {
        let name = dir.file_name().unwrap().to_str().unwrap().to_string();
        let toml = dir.join("lang.toml");
        let scm = dir.join("whence.scm");
        for f in [&toml, &scm] {
            assert!(f.is_file(), "{} is missing", f.display());
            println!("cargo:rerun-if-changed={}", f.display());
        }
        writeln!(
            table,
            "        ({:?}, tree_sitter_{}::LANGUAGE.into(), include_str!({:?}), include_str!({:?})),",
            name,
            name.replace('-', "_"),
            toml.to_str().unwrap(),
            scm.to_str().unwrap(),
        )
        .unwrap();
    }
    table.push_str("    ]\n}\n");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(&out, table).unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
}
