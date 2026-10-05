//! Clipboard and copy-to-run command text (SPEC §9: copy only, never executed).

use gp_atlas_core::command::ReadOnlyCommand;
use gp_atlas_core::manifest::Manifest;

/// Copies text to the OS clipboard.
pub fn copy(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_owned()))
        .map_err(|e| e.to_string())
}

/// POSIX shell quoting (bash/zsh). Values are already validated (§9.2), so this
/// is defence in depth.
pub fn quote_posix(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._@+-/:=,".contains(c));
    if safe {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// The command a human would run: no `--json` (§9.1 default for copy).
pub fn command_for_copy(cmd: &ReadOnlyCommand, manifest: &Manifest) -> Option<String> {
    let argv = cmd.argv(manifest).ok()?;
    let parts: Vec<String> = argv
        .iter()
        .filter(|a| a.as_str() != "--json")
        .map(|a| quote_posix(a))
        .collect();
    Some(format!("sf {}", parts.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote_posix("my-hub"), "my-hub");
        assert_eq!(quote_posix("My Pkg"), "'My Pkg'");
        assert_eq!(quote_posix("it's"), r"'it'\''s'");
        assert_eq!(quote_posix("$HOME"), "'$HOME'");
        assert_eq!(quote_posix(""), "''");
    }
}
