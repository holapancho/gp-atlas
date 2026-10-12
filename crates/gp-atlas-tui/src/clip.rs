//! Clipboard and copy-to-run command text (SPEC §9: copy only, never executed).

use std::io::Write;
use std::sync::Mutex;

use gp_atlas_core::command::ReadOnlyCommand;
use gp_atlas_core::manifest::Manifest;

/// How a copy reached the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// The OS clipboard.
    Os,
    /// An OSC 52 escape sequence: the terminal sets its own clipboard (works
    /// over SSH; needs a terminal that supports it, e.g. iTerm2, kitty, WezTerm,
    /// Windows Terminal, tmux with `set-clipboard on`).
    Terminal,
}

/// One clipboard for the whole session: on Linux (X11/Wayland) the copied
/// text is served by this process and is lost when the clipboard is dropped.
static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

/// Copies text to the OS clipboard, else through the terminal (OSC 52).
pub fn copy(text: &str) -> Result<Via, String> {
    let os = {
        let mut guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = arboard::Clipboard::new().ok();
        }
        match guard.as_mut() {
            Some(c) => c.set_text(text.to_owned()).map_err(|e| e.to_string()),
            None => Err("no OS clipboard".to_owned()),
        }
    };
    match os {
        Ok(()) => Ok(Via::Os),
        // No OS clipboard (SSH, headless): let the terminal do it.
        Err(e) if remote_or_headless() => {
            let mut out = std::io::stdout();
            out.write_all(osc52(text).as_bytes())
                .and_then(|()| out.flush())
                .map(|()| Via::Terminal)
                .map_err(|w| format!("{e}; terminal copy failed: {w}"))
        }
        Err(e) => Err(e),
    }
}

fn remote_or_headless() -> bool {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    set("SSH_TTY")
        || set("SSH_CONNECTION")
        || (cfg!(all(unix, not(target_os = "macos"))) && !set("DISPLAY") && !set("WAYLAND_DISPLAY"))
}

/// `ESC ] 52 ; c ; <base64> BEL`.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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

    #[test]
    fn osc52_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"04t5g000000YYYYAAA"), "MDR0NWcwMDAwMDBZWVlZQUFB");
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }
}
