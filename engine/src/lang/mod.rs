pub mod vocab;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

use anyhow::{Context, anyhow};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Quirks {
    /// A variable is bound once; the write step of spec M2 §3.1 is skipped.
    pub single_assignment: bool,
}

#[derive(Deserialize)]
struct LangToml {
    extensions: Vec<String>,
    #[serde(default)]
    quirks: Quirks,
}

pub struct Language {
    pub name: &'static str,
    pub ts: tree_sitter::Language,
    pub query: tree_sitter::Query,
    pub quirks: Quirks,
    pub extensions: Vec<String>,
}

pub struct Registry {
    languages: Vec<Language>,
}

impl Registry {
    pub fn embedded() -> anyhow::Result<Registry> {
        let mut languages = Vec::new();
        for (name, ts, toml_src, scm) in embedded::table() {
            let cfg: LangToml =
                toml::from_str(toml_src).with_context(|| format!("language {name}: lang.toml"))?;
            let query = tree_sitter::Query::new(&ts, scm).map_err(|e| {
                anyhow!(
                    "language {name}: query error at byte {}: {}",
                    e.offset,
                    e.message
                )
            })?;
            languages.push(Language {
                name,
                ts,
                query,
                quirks: cfg.quirks,
                extensions: cfg.extensions,
            });
        }
        Ok(Registry { languages })
    }

    pub fn for_file(&self, path: &Path) -> Option<&Language> {
        let ext = path.extension()?.to_str()?;
        self.languages
            .iter()
            .find(|l| l.extensions.iter().any(|e| e == ext))
    }

    pub fn by_name(&self, name: &str) -> Option<&Language> {
        self.languages.iter().find(|l| l.name == name)
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.languages.iter().map(|l| l.name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn embedded_registry_has_erlang_and_resolves_extension() {
        let r = Registry::embedded().unwrap();
        assert!(r.names().contains(&"erlang"));
        assert_eq!(
            r.for_file(Path::new("/p/src/a.erl")).unwrap().name,
            "erlang"
        );
        assert_eq!(r.for_file(Path::new("/p/src/a.rs")).unwrap().name, "rust");
        assert_eq!(r.for_file(Path::new("/p/src/a.go")).unwrap().name, "go");
        assert!(r.for_file(Path::new("/p/README.md")).is_none());
    }

    #[test]
    fn every_language_defines_required_captures() {
        let r = Registry::embedded().unwrap();
        for name in r.names() {
            let l = r
                .for_file(Path::new(&format!(
                    "x.{}",
                    r.by_name(name).unwrap().extensions[0]
                )))
                .unwrap();
            let have: Vec<&str> = l.query.capture_names().to_vec();
            for req in vocab::required() {
                assert!(have.contains(req), "{name} lacks @{req}");
            }
            assert!(
                !have.contains(&vocab::RETURN_CONTAINER) || have.contains(&vocab::RETURN_VALUE),
                "{name} has @{} without @{}",
                vocab::RETURN_CONTAINER,
                vocab::RETURN_VALUE
            );
        }
    }
}
