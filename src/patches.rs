use std::{collections::HashMap, fmt};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Patches(pub(crate) Vec<Patch>);

impl Patches {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, item: Patch) {
        self.0.push(item);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Patches {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut by_source: HashMap<&str, Vec<&Patch>> = HashMap::new();

        for patch in &self.0 {
            by_source.entry(&patch.source).or_default().push(patch);
        }

        for (source, patches) in &by_source {
            writeln!(f, "[{source}]")?;
            for patch in patches {
                writeln!(f, "  - {} (active: {})", patch.package, patch.active)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Patch {
    pub source: String,
    pub package: String,
    pub active: bool,
}
