use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use std::{fs, path::PathBuf};

#[derive(Debug)]
pub struct Manifest {
    content: String,
}

impl Manifest {
    pub fn new(path: Option<PathBuf>) -> Result<Self> {
        let path = if let Some(path) = path {
            path
        } else {
            let metadata = MetadataCommand::new()
                .no_deps()
                .exec()
                .context("failed to parse project's metadata")?;

            metadata
                .workspace_root
                .join("Cargo.toml")
                .into_std_path_buf()
        };

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read content of manifest at {}", path.display()))?;

        Ok(Manifest { content })
    }

    pub fn patches(&self) -> Vec<Patch> {
        let mut patches = Vec::new();
        let mut current_source: Option<String> = None;

        for raw_line in self.content.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let header = &line[1..line.len() - 1];
                if let Some(rest) = header.strip_prefix("patch.") {
                    let source = rest.trim();
                    let source = if let Some(s) =
                        source.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                    {
                        s
                    } else if let Some(s) =
                        source.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
                    {
                        s
                    } else {
                        source
                    };
                    current_source = Some(source.to_string());
                } else {
                    current_source = None;
                }
                continue;
            }

            let Some(source) = current_source.as_ref() else {
                continue;
            };

            let (active, rest) = if let Some(stripped) = line.strip_prefix('#') {
                (false, stripped.trim_start())
            } else {
                (true, line)
            };

            let Some((name, _)) = rest.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }

            patches.push(Patch {
                source: source.clone(),
                package: name.to_string(),
                active,
            });
        }

        patches
    }

    pub fn toggle(&mut self, package: &str) {
        let mut output = String::with_capacity(self.content.len());
        let mut in_patch = false;

        for (idx, raw_line) in self.content.lines().enumerate() {
            if idx > 0 {
                output.push('\n');
            }

            let trimmed = raw_line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let header = &trimmed[1..trimmed.len() - 1];
                in_patch = header.strip_prefix("patch.").is_some();
                output.push_str(raw_line);
                continue;
            }

            if !in_patch {
                output.push_str(raw_line);
                continue;
            }

            let Some(first_non_ws) = raw_line.find(|c: char| !c.is_whitespace()) else {
                output.push_str(raw_line);
                continue;
            };

            let (leading, rest) = raw_line.split_at(first_non_ws);
            let (is_commented, rest_after_hash, rest_for_parse) =
                if let Some(stripped) = rest.strip_prefix('#') {
                    (true, stripped, stripped.trim_start())
                } else {
                    (false, rest, rest)
                };

            let Some((name, _)) = rest_for_parse.split_once('=') else {
                output.push_str(raw_line);
                continue;
            };

            if name.trim() != package {
                output.push_str(raw_line);
                continue;
            }

            if is_commented {
                output.push_str(leading);
                output.push_str(rest_after_hash);
            } else {
                output.push_str(leading);
                output.push('#');
                output.push_str(rest);
            }
        }

        self.content = output;
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Patch {
    pub source: String,
    pub package: String,
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        let content = r#"[workspace]
resolver = "2"
default-members = ["my-project"]
members = [
    "my-project",
    "xtask",
]

[workspace.dependencies]
anyhow = "1"
bar = { git = "https://github.com/user/bar.git", branch = "main" }
baz = { git = "https://github.com/user/baz.git", branch = "main" }
foo = { git = "https://github.com/user/foo.git", branch = "main" }
xtask-watch = "1.6"

[patch.'https://github.com/user/bar.git']
bar = { path = "../bar" }

[patch."https://github.com/user/baz.git"]
baz = { path = "../baz" }

[patch."https://github.com/user/foo.git"]
#foo = { path = "../foo" }

[patch.crates-io]
xtask-watch = { path = "../xtask-watch" }"#;

        Manifest {
            content: content.to_string(),
        }
    }

    #[test]
    fn patches() {
        let manifest = manifest();
        let patches = manifest.patches();

        assert_eq!(
            patches,
            vec![
                Patch {
                    source: "https://github.com/user/bar.git".to_string(),
                    package: "bar".to_string(),
                    active: true,
                },
                Patch {
                    source: "https://github.com/user/baz.git".to_string(),
                    package: "baz".to_string(),
                    active: true,
                },
                Patch {
                    source: "https://github.com/user/foo.git".to_string(),
                    package: "foo".to_string(),
                    active: false,
                },
                Patch {
                    source: "crates-io".to_string(),
                    package: "xtask-watch".to_string(),
                    active: true,
                },
            ]
        );
    }

    #[test]
    fn toggle_active_patch() {
        let mut manifest = manifest();
        manifest.toggle("bar");

        assert_eq!(
            manifest.patches(),
            vec![
                Patch {
                    source: "https://github.com/user/bar.git".to_string(),
                    package: "bar".to_string(),
                    active: false,
                },
                Patch {
                    source: "https://github.com/user/baz.git".to_string(),
                    package: "baz".to_string(),
                    active: true,
                },
                Patch {
                    source: "https://github.com/user/foo.git".to_string(),
                    package: "foo".to_string(),
                    active: false,
                },
                Patch {
                    source: "crates-io".to_string(),
                    package: "xtask-watch".to_string(),
                    active: true,
                },
            ]
        );
    }

    #[test]
    fn toggle_inactive_patch() {
        let mut manifest = manifest();
        manifest.toggle("foo");

        assert_eq!(
            manifest.patches(),
            vec![
                Patch {
                    source: "https://github.com/user/bar.git".to_string(),
                    package: "bar".to_string(),
                    active: true,
                },
                Patch {
                    source: "https://github.com/user/baz.git".to_string(),
                    package: "baz".to_string(),
                    active: true,
                },
                Patch {
                    source: "https://github.com/user/foo.git".to_string(),
                    package: "foo".to_string(),
                    active: true,
                },
                Patch {
                    source: "crates-io".to_string(),
                    package: "xtask-watch".to_string(),
                    active: true,
                },
            ]
        );
    }
}
