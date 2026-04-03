use crate::{Patch, Patches};
use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Manifest {
    path: PathBuf,
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

        Ok(Manifest { path, content })
    }

    pub fn patches(&self) -> Patches {
        let mut patches = Patches::new();
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

    pub fn add(
        &mut self,
        source: Option<String>,
        package: impl AsRef<str>,
        path: impl AsRef<Path>,
    ) {
        let source = source.unwrap_or_else(|| "crates-io".to_string());
        let path = path.as_ref();
        let package = package.as_ref();
        let header = if source == "crates-io" {
            "[patch.crates-io]".to_string()
        } else {
            format!("[patch.\"{}\"]", source)
        };
        let new_line = format!("{package} = {{ path = \"{}\" }}", path.display());

        let mut lines_out: Vec<String> = Vec::new();
        let mut in_patch = false;
        let mut in_target = false;
        let mut target_section_found = false;
        let mut target_entry_found = false;

        for raw_line in self.content.lines() {
            let trimmed = raw_line.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if in_target && !target_entry_found {
                    lines_out.push(new_line.clone());
                    target_entry_found = true;
                }

                let header = &trimmed[1..trimmed.len() - 1];
                if let Some(rest) = header.strip_prefix("patch.") {
                    let src = rest.trim();
                    let src =
                        if let Some(s) = src.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                            s
                        } else if let Some(s) =
                            src.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
                        {
                            s
                        } else {
                            src
                        };
                    in_patch = true;
                    in_target = src == source;
                    if in_target {
                        target_section_found = true;
                    }
                } else {
                    in_patch = false;
                    in_target = false;
                }

                lines_out.push(raw_line.to_string());
                continue;
            }

            if !in_patch {
                lines_out.push(raw_line.to_string());
                continue;
            }

            let Some(first_non_ws) = raw_line.find(|c: char| !c.is_whitespace()) else {
                lines_out.push(raw_line.to_string());
                continue;
            };

            let (leading, rest) = raw_line.split_at(first_non_ws);
            let rest_for_parse = if let Some(stripped) = rest.strip_prefix('#') {
                stripped.trim_start()
            } else {
                rest
            };

            let Some((name, _)) = rest_for_parse.split_once('=') else {
                lines_out.push(raw_line.to_string());
                continue;
            };

            if name.trim() == package && in_target {
                lines_out.push(format!("{leading}{new_line}"));
                target_entry_found = true;
                continue;
            }

            lines_out.push(raw_line.to_string());
        }

        if in_target && !target_entry_found {
            lines_out.push(new_line.clone());
        }

        if !target_section_found {
            if !lines_out.last().map(|l| l.is_empty()).unwrap_or(true) {
                lines_out.push(String::new());
            }
            lines_out.push(header);
            lines_out.push(new_line);
        }

        self.content = lines_out.join("\n");
    }

    pub fn toggle(&mut self, package: impl AsRef<str>) {
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

            if name.trim() != package.as_ref() {
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

    pub fn remove(&mut self, package: impl AsRef<str>) {
        let mut lines_out: Vec<String> = Vec::new();

        let mut in_patch = false;
        let mut current_section: Vec<String> = Vec::new();
        let mut section_has_entries = false;
        let mut removed = false;

        for raw_line in self.content.lines() {
            let trimmed = raw_line.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if in_patch {
                    if section_has_entries {
                        lines_out.append(&mut current_section);
                    }
                    current_section.clear();
                    section_has_entries = false;
                }

                let header = &trimmed[1..trimmed.len() - 1];
                in_patch = header.strip_prefix("patch.").is_some();

                if in_patch {
                    current_section.push(raw_line.to_string());
                } else {
                    lines_out.push(raw_line.to_string());
                }
                continue;
            }

            if !in_patch {
                lines_out.push(raw_line.to_string());
                continue;
            }

            let Some(first_non_ws) = raw_line.find(|c: char| !c.is_whitespace()) else {
                current_section.push(raw_line.to_string());
                continue;
            };

            let (_, rest) = raw_line.split_at(first_non_ws);
            let rest_for_parse = if let Some(stripped) = rest.strip_prefix('#') {
                stripped.trim_start()
            } else {
                rest
            };

            let Some((name, _)) = rest_for_parse.split_once('=') else {
                current_section.push(raw_line.to_string());
                continue;
            };

            if name.trim() == package.as_ref() {
                removed = true;
                continue;
            }

            current_section.push(raw_line.to_string());
            section_has_entries = true;
        }

        if in_patch && section_has_entries {
            lines_out.append(&mut current_section);
        }

        if removed {
            self.content = lines_out.join("\n");
        }
    }

    pub fn write(self) -> Result<()> {
        fs::write(&self.path, self.content)
            .with_context(|| format!("failed to write manifest at {}", self.path.display()))
    }
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
            path: PathBuf::from("Cargo.toml"),
            content: content.to_string(),
        }
    }

    #[test]
    fn patches() {
        let manifest = manifest();
        let patches = manifest.patches();

        assert_eq!(
            patches,
            Patches(vec![
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
            ])
        );
    }

    #[test]
    fn add_crates_io() {
        let mut manifest = manifest();
        let path = PathBuf::from("../rab");
        manifest.add(None, "rab", &path);

        assert_eq!(
            manifest.patches(),
            Patches(vec![
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
                Patch {
                    source: "crates-io".to_string(),
                    package: "rab".to_string(),
                    active: true,
                },
            ])
        );
        assert!(manifest.content.contains("rab = { path = \"../rab\" }"));
    }

    #[test]
    fn add_repository() {
        let mut manifest = manifest();
        let path = PathBuf::from("../rab");
        manifest.add(
            Some("https://github.com/user/rab.git".to_string()),
            "rab",
            &path,
        );

        assert_eq!(
            manifest.patches(),
            Patches(vec![
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
                Patch {
                    source: "https://github.com/user/rab.git".to_string(),
                    package: "rab".to_string(),
                    active: true,
                },
            ])
        );
        assert!(manifest.content.contains("rab = { path = \"../rab\" }"));
    }

    #[test]
    fn toggle_active_patch() {
        let mut manifest = manifest();
        manifest.toggle("bar");

        assert_eq!(
            manifest.patches(),
            Patches(vec![
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
            ])
        );
    }

    #[test]
    fn toggle_inactive_patch() {
        let mut manifest = manifest();
        manifest.toggle("foo");

        assert_eq!(
            manifest.patches(),
            Patches(vec![
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
            ])
        );
    }

    #[test]
    fn remove() {
        let mut manifest = manifest();
        manifest.remove("baz");

        assert_eq!(
            manifest.patches(),
            Patches(vec![
                Patch {
                    source: "https://github.com/user/bar.git".to_string(),
                    package: "bar".to_string(),
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
            ])
        );
        assert!(
            !manifest
                .content
                .contains("[patch.\"https://github.com/user/baz.git\"]")
        );
    }
}
