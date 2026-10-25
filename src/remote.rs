//! Git remotes: which repository on a code host a clone belongs to.

/// `owner/repo` from a git remote URL: `git@github.com:owner/repo.git`,
/// `https://github.com/owner/repo`, or a proxy's `http://proxy@127.0.0.1:1234/git/owner/repo`
/// as in Claude Code cloud sessions. The last two path segments, without `.git`.
pub fn repo_full_name(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let path = match remote.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map(|(_, p)| p)?,
        // scp-like: user@host:path
        None => remote.split_once(':').map(|(_, p)| p)?,
    };
    let segments: Vec<&str> = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let [.., owner, repo] = segments.as_slice() else {
        return None;
    };
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    (!owner.is_empty() && !repo.is_empty()).then(|| format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::repo_full_name;

    #[test]
    fn full_names_from_remotes() {
        for (remote, name) in [
            ("git@github.com:octo-org/app.git", Some("octo-org/app")),
            ("https://github.com/octo-org/app", Some("octo-org/app")),
            ("https://github.com/octo-org/app.git/", Some("octo-org/app")),
            (
                "ssh://git@github.com/octo-org/app.git",
                Some("octo-org/app"),
            ),
            (
                "http://local_proxy@127.0.0.1:41921/git/octo-org/app",
                Some("octo-org/app"),
            ),
            ("/home/me/app", None),
            ("app", None),
        ] {
            assert_eq!(repo_full_name(remote).as_deref(), name, "{remote}");
        }
    }
}
