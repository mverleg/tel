use std::path::PathBuf;
use serde::Deserialize;
use serde::Serialize;

/// Need distinction between simple names and fully-qualified names in the future,
/// but for now everything is simple for the demo.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Name {
    name: String,
}

impl Name {
    pub fn of(name: impl Into<String>) -> Name {
        Name { name: name.into() }
    }

    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Path {
    path: PathBuf,
}

impl Path {
    pub fn of(path: impl Into<PathBuf>) -> Path {
        Path { path: path.into() }
    }
}
