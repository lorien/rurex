//! Vendored engine code (see `pikevm.rs` for provenance and the
//! modification list).

pub(crate) mod pikevm;
pub(crate) mod sparse_set;

/// The `regex-automata` version the vendored `pikevm.rs` was taken
/// from. Must equal the version resolved in `Cargo.lock`; the test
/// below enforces it so a regex bump cannot silently leave the
/// vendored engine behind.
#[allow(dead_code)] // read by the lockfile-sync test
pub(crate) const VENDORED_REGEX_AUTOMATA: &str = "0.4.18";

#[cfg(test)]
mod tests {
    use super::VENDORED_REGEX_AUTOMATA;

    /// The lockfile must resolve exactly one `regex-automata`, and it
    /// must match the vendored copy's version. A regex bump updates
    /// the lock and turns this red until the vendored file is
    /// re-vendored from the new version (see pikevm.rs header).
    #[test]
    fn vendored_pikevm_version_matches_lockfile() {
        let lock = include_str!("../../Cargo.lock");
        let mut versions = Vec::new();
        let mut lines = lock.lines().peekable();
        while let Some(line) = lines.next() {
            if line.trim() == "name = \"regex-automata\"" {
                for next in lines.by_ref() {
                    if let Some(v) = next.trim().strip_prefix("version = ") {
                        versions.push(v.trim_matches('"').to_string());
                        break;
                    }
                }
            }
        }
        assert_eq!(
            versions.len(),
            1,
            "expected exactly one regex-automata in Cargo.lock, got {versions:?}"
        );
        assert_eq!(
            versions[0], VENDORED_REGEX_AUTOMATA,
            "Cargo.lock has regex-automata {} but the vendored pikevm.rs \
             was taken from {} — re-vendor it (see src/vendor/pikevm.rs \
             header) and re-apply the marked [rurex] modifications",
            versions[0], VENDORED_REGEX_AUTOMATA,
        );
    }
}
