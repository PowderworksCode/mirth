//! What to watch, read from the file `MIRTH_WATCH` names.
//!
//! ```toml
//! [scope]
//! crates = ["rustc_metadata"]   # crates to instrument; empty: the packages Cargo was asked to build
//!
//! [[frame]]                     # functions whose calls attribute what happens inside them
//! match = "rustc_metadata::rmeta::encoder::encode_metadata"
//!
//! [[call]]                      # calls to record
//! match = "std::fs::rename"
//! mode = "log"                  # "log": each one, timestamped, written at once; "count": counted
//! capture = [0, 1]              # the arguments to write down
//! point = true                  # also a point where MIRTH_CRASH can stop the process
//!
//! [statics]                     # touches of mutable and interior-mutable statics
//! mode = "count"
//! ```
//!
//! A pattern is a function's path, as rustc prints it with the crate's name
//! first, where `*` matches any run of characters.

use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub scope: Scope,
    #[serde(default, rename = "frame")]
    pub frames: Vec<Frame>,
    #[serde(default, rename = "call")]
    pub calls: Vec<Call>,
    pub statics: Option<Statics>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    #[serde(default)]
    pub crates: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    #[serde(rename = "match")]
    pub pattern: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    #[serde(rename = "match")]
    pub pattern: String,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub capture: Vec<usize>,
    #[serde(default)]
    pub point: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statics {
    #[serde(default)]
    pub mode: Mode,
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Count,
    Log,
}

impl Mode {
    pub fn code(self) -> u64 {
        match self {
            Mode::Count => 0,
            Mode::Log => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Count => "count",
            Mode::Log => "log",
        }
    }
}

impl Config {
    pub fn load() -> Option<Config> {
        let path = std::env::var_os("MIRTH_WATCH")?;
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("mirth-watch: reading {}: {error}", path.to_string_lossy())
        });
        Some(
            toml::from_str(&text)
                .unwrap_or_else(|error| panic!("mirth-watch: {}: {error}", path.to_string_lossy())),
        )
    }

    pub fn is_frame(&self, path: &str) -> bool {
        self.frames
            .iter()
            .any(|frame| matches(&frame.pattern, path))
    }

    pub fn call(&self, path: &str) -> Option<&Call> {
        self.calls.iter().find(|call| matches(&call.pattern, path))
    }
}

/// Whether `path` matches `pattern`, where `*` matches any run of characters.
pub fn matches(pattern: &str, path: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = path.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn patterns() {
        assert!(matches("std::fs::rename", "std::fs::rename"));
        assert!(!matches("std::fs::rename", "std::fs::rename_x"));
        assert!(matches("std::fs::*", "std::fs::write"));
        assert!(matches(
            "*::encode_metadata",
            "rustc_metadata::rmeta::encoder::encode_metadata"
        ));
        assert!(matches("a::*::c", "a::b::c"));
        assert!(!matches("a::*::c", "a::b::d"));
        assert!(matches("*", "anything"));
    }
}
