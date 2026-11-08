//! Strict, std-only configuration and URL validation.
use std::{fs, io, path::Path};

#[derive(Debug, Clone)]
pub struct Config {
    pub start_url: String,
    pub allowed_hosts: Vec<String>,
    pub blocked_processes: Vec<String>,
    pub blocked_app_paths: Vec<String>,
    pub require_network_filter: bool,
}

impl Config {
    pub fn load(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        Self::parse(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
    pub fn parse(contents: &str) -> Result<Self, String> {
        let mut config = Self {
            start_url: String::new(), allowed_hosts: Vec::new(),
            blocked_processes: Vec::new(), blocked_app_paths: Vec::new(),
            require_network_filter: false,
        };
        for (line_no, raw) in contents.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let (key, val) = line.split_once('=')
                .ok_or_else(|| format!("line {}: expected key=value", line_no + 1))?;
            let value = val.trim();
            match key.trim() {
                "start_url" => config.start_url = value.to_string(),
                "allowed_host" => {
                    if !is_valid_host_pattern(value) { return Err(format!("line {}: invalid host pattern", line_no + 1)); }
                    config.allowed_hosts.push(value.to_ascii_lowercase());
                }
                "blocked_process" => {
                    // Application image names, never PID or arbitrary process termination.
                    if value.is_empty() || value.contains('/') || value.contains('\\') {
                        return Err(format!("line {}: expected executable name", line_no + 1));
                    }
                    config.blocked_processes.push(value.to_ascii_lowercase());
                }
                "blocked_app_path" => {
                    if !value.is_empty() { config.blocked_app_paths.push(value.to_string()); }
                }
                "require_network_filter" => config.require_network_filter = match value {
                    "true" => true, "false" => false,
                    _ => return Err(format!("line {}: require_network_filter must be true/false", line_no + 1)),
                },
                _ => return Err(format!("line {}: unknown option '{}'", line_no + 1, key.trim())),
            }
        }
        if config.allowed_hosts.is_empty() { return Err("at least one allowed_host is required".into()); }
        if !config.allows_url(&config.start_url) {
            return Err("start_url must be an HTTPS URL on an allowed host".into());
        }
        if config.require_network_filter && config.blocked_app_paths.is_empty() {
            return Err("require_network_filter=true requires blocked_app_path entries".into());
        }
        Ok(config)
    }
    pub fn allows_url(&self, url: &str) -> bool {
        let Some(host) = extract_https_host(url) else { return false; };
        self.allowed_hosts.iter().any(|pattern| match pattern.strip_prefix("*.") {
            Some(suffix) => host.len() > suffix.len() + 1
                && host.ends_with(suffix)
                && host.as_bytes()[host.len() - suffix.len() - 1] == b'.',
            None => pattern == &host,
        })
    }
}

pub fn extract_https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.contains('%') || authority.contains('\\') { return None; }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h,p)) if !h.contains(':') => (h, Some(p)),
        Some(_) => return None, // IPv6 and ambiguous authorities unsupported by this policy.
        None => (authority, None),
    };
    if port.is_some_and(|p| p != "443") || !is_valid_host_pattern(host) || host.starts_with("*.") {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

pub fn is_valid_host_pattern(value: &str) -> bool {
    let host = value.strip_prefix("*.").unwrap_or(value);
    if host.is_empty() || host.len() > 253 || !host.contains('.') || host.ends_with('.') { return false; }
    host.split('.').all(|label| {
        !label.is_empty() && label.len() <= 63
        && !label.starts_with('-') && !label.ends_with('-')
        && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn exact_host_is_not_a_suffix_match() {
        let c = Config::parse("start_url=https://exam.school.edu/\nallowed_host=exam.school.edu\n").unwrap();
        assert!(c.allows_url("https://exam.school.edu/quiz"));
        assert!(!c.allows_url("https://exam.school.edu.evil.net/"));
        assert!(!c.allows_url("https://evil.net@exam.school.edu/"));
        assert!(!c.allows_url("http://exam.school.edu/"));
        assert!(!c.allows_url("https://exam.school.edu:444/"));
    }
    #[test] fn wildcard_requires_subdomain() {
        let c = Config::parse("start_url=https://quiz.school.edu/\nallowed_host=*.school.edu\n").unwrap();
        assert!(c.allows_url("https://cdn.school.edu/image.png"));
        assert!(!c.allows_url("https://school.edu/"));
        assert!(!c.allows_url("https://evil-school.edu/"));
    }
    #[test] fn never_allow_local_schemes() {
        let c = Config::parse("start_url=https://school.edu/\nallowed_host=school.edu\n").unwrap();
        for v in ["file:///C:/Users", "javascript:alert(1)", "about:blank", "data:text/html,A"] {
            assert!(!c.allows_url(v));
        }
    }
}
