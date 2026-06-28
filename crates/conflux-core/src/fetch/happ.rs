//! Resolve Happ encrypted subscription links (`happ://crypt*`) to fetchable URLs.

use std::path::{Path, PathBuf};
use std::process::Command;

use conflux_protocol::ConfluxError;

/// Decrypt or pass through a subscription URL before HTTP fetch.
pub fn resolve_subscription_url(url: &str) -> Result<String, ConfluxError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(ConfluxError::InvalidUrl("URL is empty".into()));
    }

    if !trimmed.starts_with("happ://") {
        return Ok(trimmed.to_string());
    }

    let normalized = normalize_happ_link(trimmed);
    decrypt_with_helper(&normalized)
}

/// Trim whitespace and strip Happ routing suffixes (`=ff`, `=profile`, …).
pub fn normalize_happ_link(url: &str) -> String {
    let mut link: String = url.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(stripped) = strip_happ_routing_suffix(&link) {
        link = stripped;
    }
    link
}

fn strip_happ_routing_suffix(link: &str) -> Option<String> {
    let slash = link.rfind('/')?;
    let prefix = &link[..=slash];
    let payload = &link[slash + 1..];
    if payload.is_empty() {
        return None;
    }

    // Happ may append `=<routing-id>` after the encrypted payload.
    let eq = payload.rfind('=')?;
    let suffix = &payload[eq + 1..];
    if suffix.is_empty() || suffix.len() > 32 || !suffix.chars().all(is_routing_suffix_char) {
        return None;
    }

    let body = &payload[..eq];
    if body.is_empty() {
        return None;
    }

    Some(format!("{prefix}{body}"))
}

fn is_routing_suffix_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')
}

fn decrypt_with_helper(link: &str) -> Result<String, ConfluxError> {
    let helper = locate_happ_decrypt_helper().ok_or_else(|| {
        ConfluxError::InvalidUrl(
            "happ:// links require happ-decrypt.exe in engines/ next to confluxd".into(),
        )
    })?;

    let output = Command::new(&helper).arg(link).output().map_err(|err| {
        ConfluxError::InvalidUrl(format!(
            "failed to run happ decrypt helper '{}': {err}",
            helper.display()
        ))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(error) = parse_helper_error(&stdout) {
        return Err(ConfluxError::InvalidUrl(format!(
            "happ link decrypt failed: {error}. Copy the full link from Happ without line breaks; \
             try an HTTPS subscription URL if the problem persists"
        )));
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            stdout.trim().to_string()
        } else {
            stderr
        };
        return Err(ConfluxError::InvalidUrl(format!(
            "happ decrypt helper failed: {detail}"
        )));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| {
        ConfluxError::InvalidUrl(format!("happ decrypt output is not utf-8: {err}"))
    })?;

    parse_helper_output(&stdout).map_err(|err| {
        ConfluxError::InvalidUrl(format!(
            "{err}. Copy the full happ:// link from Happ; HTTPS subscription URLs also work"
        ))
    })
}

fn parse_helper_error(stdout: &str) -> Option<String> {
    let mut in_error = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("error") {
            in_error = true;
            continue;
        }
        if in_error && !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn parse_helper_output(stdout: &str) -> Result<String, ConfluxError> {
    let mut in_result = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("result") {
            in_result = true;
            continue;
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return Ok(trimmed.to_string());
        }

        if in_result && !trimmed.is_empty() && !trimmed.contains(':') {
            continue;
        }
    }

    stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("http://") || line.starts_with("https://"))
        .last()
        .map(str::to_string)
        .ok_or_else(|| {
            ConfluxError::InvalidUrl(
                "happ decrypt helper did not return an http(s) subscription URL".into(),
            )
        })
}

fn locate_happ_decrypt_helper() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CONFLUX_HAPP_DECRYPT") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
            dirs.push(parent.join("engines"));
            if let Some(grand) = parent.parent() {
                dirs.push(grand.join("engines"));
            }
        }
    }

    for dir in dirs {
        if let Some(path) = find_helper_in_dir(&dir) {
            return Some(path);
        }
    }

    None
}

fn find_helper_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in [
        "happ-decrypt.exe",
        "windows-x64_x86.exe",
        "happ-decrypt",
        "linux-x64_x86",
    ] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{normalize_happ_link, parse_helper_output, resolve_subscription_url};

    #[test]
    fn passes_through_https_urls() {
        let url = "https://example.com/sub";
        assert_eq!(resolve_subscription_url(url).expect("url"), url);
    }

    #[test]
    fn strips_routing_suffix() {
        assert_eq!(
            normalize_happ_link("happ://crypt5/abc123=ff"),
            "happ://crypt5/abc123"
        );
    }

    #[test]
    fn parses_helper_result_block() {
        let stdout = "Input\n  mode: crypt5\nResult\n  https://example.com/sub/abc\n";
        assert_eq!(
            parse_helper_output(stdout).expect("parsed"),
            "https://example.com/sub/abc"
        );
    }

    #[test]
    fn parses_helper_error_block() {
        let stdout = "Input\n  mode: crypt5\nError\n  crypt5 segment length is missing\n";
        assert_eq!(
            super::parse_helper_error(stdout).as_deref(),
            Some("crypt5 segment length is missing")
        );
    }
}
