//! Shell quoting for **copied** commands only (SPEC §9.3). GP Atlas never runs
//! a shell: the runner passes argv directly. Values are already validated
//! (§9.2, which forbids most metacharacters), so quoting is defence in depth.

/// Target shell of a copied command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shell {
    /// bash, zsh and other POSIX shells.
    #[default]
    Posix,
    PowerShell,
    /// Windows `cmd.exe`.
    Cmd,
}

impl Shell {
    pub const ALL: [Self; 3] = [Self::Posix, Self::PowerShell, Self::Cmd];

    pub fn label(self) -> &'static str {
        match self {
            Self::Posix => "bash/zsh",
            Self::PowerShell => "PowerShell",
            Self::Cmd => "cmd",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Posix => Self::PowerShell,
            Self::PowerShell => Self::Cmd,
            Self::Cmd => Self::Posix,
        }
    }

    /// Line continuation for multi-line commands.
    pub fn continuation(self) -> &'static str {
        match self {
            Self::Posix => " \\",
            Self::PowerShell => " `",
            Self::Cmd => " ^",
        }
    }

    /// `cd <dir>` followed by the separator that runs the next command only
    /// if `cd` succeeded.
    pub fn cd_prefix(self, dir: &str) -> Result<String, QuoteError> {
        Ok(match self {
            Self::Posix => format!("cd {} && ", self.quote(dir)?),
            // `;` works in Windows PowerShell 5 and PowerShell 7 alike.
            Self::PowerShell => format!("Set-Location -LiteralPath {}; ", self.quote(dir)?),
            Self::Cmd => format!("cd /d {} && ", self.quote(dir)?),
        })
    }

    /// Quotes one argument for this shell.
    pub fn quote(self, arg: &str) -> Result<String, QuoteError> {
        if arg.chars().any(char::is_control) {
            return Err(QuoteError(arg.to_owned()));
        }
        Ok(match self {
            Self::Posix => posix(arg),
            Self::PowerShell => powershell(arg),
            Self::Cmd => cmd(arg)?,
        })
    }
}

/// A value that cannot be quoted safely for the chosen shell.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot quote {0:?} safely for this shell")]
pub struct QuoteError(pub String);

fn plain(arg: &str, extra: &str) -> bool {
    !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/:=".contains(c) || extra.contains(c))
}

/// Single quotes; an embedded `'` becomes `'\''`.
fn posix(arg: &str) -> String {
    if plain(arg, "@+,") {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// Single quotes (no expansion); an embedded `'` is doubled. `,` (array
/// operator) and a leading `@` (splatting) are quoted too.
fn powershell(arg: &str) -> String {
    // PowerShell also treats the typographic quotes ‘ ’ ‚ ‛ as `'`.
    if plain(arg, "+") && !arg.starts_with('-') || plain(arg, "+") && arg.starts_with("--") {
        arg.to_owned()
    } else {
        let mut s = String::with_capacity(arg.len() + 2);
        s.push('\'');
        for c in arg.chars() {
            if matches!(c, '\'' | '‘' | '’' | '‚' | '‛') {
                s.push(c);
            }
            s.push(c);
        }
        s.push('\'');
        s
    }
}

/// Double quotes. `cmd` cannot escape `"` inside quotes, and `%` / `!` expand
/// even there, so those are refused (validated values never contain them).
fn cmd(arg: &str) -> Result<String, QuoteError> {
    if arg.contains(['"', '%', '!']) || arg.ends_with('\\') {
        return Err(QuoteError(arg.to_owned()));
    }
    Ok(if plain(arg, "@") {
        arg.to_owned()
    } else {
        format!("\"{arg}\"")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_table() {
        let q = |a| Shell::Posix.quote(a).unwrap();
        assert_eq!(q("my-hub"), "my-hub");
        assert_eq!(q("user.name+dev@example.com"), "user.name+dev@example.com");
        assert_eq!(q("0Ho1,0Ho2"), "0Ho1,0Ho2");
        assert_eq!(q("My Pkg"), "'My Pkg'");
        assert_eq!(q("CreatedDate DESC"), "'CreatedDate DESC'");
        assert_eq!(q("it's"), r"'it'\''s'");
        assert_eq!(q("$HOME"), "'$HOME'");
        assert_eq!(q("a`b"), "'a`b'");
        assert_eq!(q("a;b&c"), "'a;b&c'");
        assert_eq!(q("100%"), "'100%'");
        assert_eq!(q("ünï"), "'ünï'");
        assert_eq!(q(""), "''");
        assert_eq!(q(r"dir\"), r"'dir\'");
    }

    #[test]
    fn powershell_table() {
        let q = |a| Shell::PowerShell.quote(a).unwrap();
        assert_eq!(q("my-hub"), "my-hub");
        assert_eq!(q("--target-dev-hub"), "--target-dev-hub");
        assert_eq!(q("user@example.com"), "'user@example.com'");
        assert_eq!(q("0Ho1,0Ho2"), "'0Ho1,0Ho2'");
        assert_eq!(q("My Pkg"), "'My Pkg'");
        assert_eq!(q("it's"), "'it''s'");
        assert_eq!(q("it’s"), "'it’’s'");
        assert_eq!(q("$HOME"), "'$HOME'");
        assert_eq!(q("a`b"), "'a`b'");
        assert_eq!(q("a;b&c"), "'a;b&c'");
        assert_eq!(q(""), "''");
        assert_eq!(q("-x"), "'-x'");
    }

    #[test]
    fn cmd_table() {
        let q = |a| Shell::Cmd.quote(a);
        assert_eq!(q("my-hub").unwrap(), "my-hub");
        assert_eq!(q("user@example.com").unwrap(), "user@example.com");
        assert_eq!(q("0Ho1,0Ho2").unwrap(), "\"0Ho1,0Ho2\"");
        assert_eq!(q("My Pkg").unwrap(), "\"My Pkg\"");
        assert_eq!(q("a&b^c<d>e|f").unwrap(), "\"a&b^c<d>e|f\"");
        assert_eq!(q("it's").unwrap(), "\"it's\"");
        assert_eq!(q("").unwrap(), "\"\"");
        for bad in ["a\"b", "100%", "%PATH%", "wow!", r"dir\"] {
            assert!(q(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn control_characters_are_refused() {
        for s in Shell::ALL {
            assert!(s.quote("a\nb").is_err());
            assert!(s.quote("a\u{1b}[31m").is_err());
        }
    }

    #[test]
    fn cd_prefixes() {
        assert_eq!(
            Shell::Posix.cd_prefix("/w/my proj").unwrap(),
            "cd '/w/my proj' && "
        );
        assert_eq!(
            Shell::PowerShell.cd_prefix(r"C:\w\proj").unwrap(),
            r"Set-Location -LiteralPath 'C:\w\proj'; "
        );
        assert_eq!(
            Shell::Cmd.cd_prefix(r"C:\w\my proj").unwrap(),
            r#"cd /d "C:\w\my proj" && "#
        );
    }
}
