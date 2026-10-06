//! Making a record comparable across runs and machines: paths relative to
//! the project, and no hashes, random names or process ids.

use regex::Regex;

pub struct Normalize {
    roots: Vec<(String, String)>,
    hash: Regex,
    hash_directory: Regex,
    random: Regex,
}

impl Normalize {
    /// `roots` are prefixes to shorten, longest first, with what to show
    /// instead: the target directory as `target`, the project as `.`.
    pub fn new(mut roots: Vec<(String, String)>) -> Normalize {
        roots.retain(|(prefix, _)| !prefix.is_empty());
        roots.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        Normalize {
            roots,
            hash: Regex::new(r"-[0-9a-f]{16}\b").expect("a pattern"),
            hash_directory: Regex::new(r"/[0-9a-f]{16}/").expect("a pattern"),
            random: Regex::new(r"(^|/)(rmeta|rustc|\.tmp)[A-Za-z0-9]{6}").expect("a pattern"),
        }
    }

    pub fn path(&self, text: &str) -> String {
        let mut text = text.replace('\\', "/");
        for (prefix, instead) in &self.roots {
            let prefix = prefix.replace('\\', "/");
            if let Some(rest) = text.strip_prefix(&prefix) {
                text = format!("{instead}{rest}");
                break;
            }
        }
        let text = self.hash.replace_all(&text, "-#");
        let text = self.hash_directory.replace_all(&text, "/#/");
        self.random.replace_all(&text, "$1$2*").into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::Normalize;

    #[test]
    fn paths() {
        let normalize = Normalize::new(vec![
            ("/work/p".to_owned(), ".".to_owned()),
            ("/work/p/target".to_owned(), "target".to_owned()),
        ]);
        assert_eq!(
            normalize.path("/work/p/target/debug/deps/libbase-0123456789abcdef.rmeta"),
            "target/debug/deps/libbase-#.rmeta"
        );
        assert_eq!(
            normalize.path("/work/p/target/debug/deps/rmetaAb3XyZ/full.rmeta"),
            "target/debug/deps/rmeta*/full.rmeta"
        );
        assert_eq!(
            normalize.path(
                "/work/p/target/debug/build/base/e2ac2a45747a0cc6/out/.tmphvI0ZT.temp-archive"
            ),
            "target/debug/build/base/#/out/.tmp*.temp-archive"
        );
        assert_eq!(normalize.path("/elsewhere/x"), "/elsewhere/x");
    }
}
