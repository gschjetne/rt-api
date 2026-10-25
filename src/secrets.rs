//! Text that looks like a credential. Review documents, the log, answers and text
//! attachments are read by everyone with access to the repository, and kept in every
//! revision, so the server refuses them when they contain one; their author rewrites the
//! line.
//!
//! Two kinds of finding: tokens whose format gives them away (prefixes like `ghp_`, `AKIA`,
//! `sk_live_`), and a long, random-looking value assigned to a name like `password` or
//! `api_key`. Random strings on their own (hashes, ids) are left alone: review documents
//! are full of them.

/// What a secret-looking word is, and the line it is on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub line: usize,
    pub kind: &'static str,
}

fn all(s: &str, ok: impl Fn(char) -> bool) -> bool {
    !s.is_empty() && s.chars().all(ok)
}

fn kind_of(word: &str) -> Option<&'static str> {
    let alnum = |c: char| c.is_ascii_alphanumeric();
    if let Some(rest) = word.strip_prefix("rt_")
        && rest.len() == 64
        && all(rest, |c| c.is_ascii_hexdigit())
    {
        return Some("review-tools agent token");
    }
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        if let Some(rest) = word.strip_prefix(prefix)
            && rest.len() >= 36
            && all(rest, alnum)
        {
            return Some("GitHub token");
        }
    }
    if let Some(rest) = word.strip_prefix("github_pat_")
        && rest.len() >= 22
        && all(rest, |c| alnum(c) || c == '_')
    {
        return Some("GitHub token");
    }
    for prefix in ["AKIA", "ASIA"] {
        if let Some(rest) = word.strip_prefix(prefix)
            && rest.len() == 16
            && all(rest, |c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Some("AWS access key");
        }
    }
    if let Some(rest) = word.strip_prefix("sk-ant-")
        && rest.len() >= 20
    {
        return Some("Anthropic API key");
    }
    for prefix in ["xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-"] {
        if let Some(rest) = word.strip_prefix(prefix)
            && rest.len() >= 10
        {
            return Some("Slack token");
        }
    }
    if let Some(rest) = word.strip_prefix("AIza")
        && rest.len() == 35
        && all(rest, |c| alnum(c) || c == '_' || c == '-')
    {
        return Some("Google API key");
    }
    for prefix in ["sk_live_", "rk_live_", "sk_test_", "rk_test_", "whsec_"] {
        if let Some(rest) = word.strip_prefix(prefix)
            && rest.len() >= 24
            && all(rest, alnum)
        {
            return Some("Stripe key");
        }
    }
    for prefix in ["sk-proj-", "sk-svcacct-", "sk-admin-"] {
        if let Some(rest) = word.strip_prefix(prefix)
            && rest.len() >= 20
        {
            return Some("OpenAI API key");
        }
    }
    if let Some(rest) = word.strip_prefix("sk-")
        && rest.len() == 48
        && all(rest, alnum)
    {
        return Some("OpenAI API key");
    }
    if let Some(rest) = word.strip_prefix("glpat-")
        && rest.len() >= 20
    {
        return Some("GitLab token");
    }
    if let Some(rest) = word.strip_prefix("npm_")
        && rest.len() == 36
        && all(rest, alnum)
    {
        return Some("npm token");
    }
    None
}

/// Bits of entropy per character, from the characters' frequencies.
fn entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let n = s.chars().count() as f64;
    counts
        .values()
        .map(|&k| {
            let p = k as f64 / n;
            -p * p.log2()
        })
        .sum()
}

/// Names under which a value is a secret.
const SECRET_NAMES: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "access_key",
    "access-key",
    "private_key",
    "client_secret",
    "credential",
];

/// A long, random-looking value assigned to a secret's name: `AWS_SECRET_ACCESS_KEY=…`,
/// `"password": "…"`, `api_key: …`.
fn assigned_secret(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    for name in SECRET_NAMES {
        for (at, _) in lower.match_indices(name) {
            let rest = &line[at + name.len()..];
            // The rest of the name (`_KEY`, `"`), then `:` or `=`.
            let rest =
                rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || "_-\"'".contains(c));
            let Some(rest) = rest
                .trim_start()
                .strip_prefix([':', '='])
                .map(|r| r.trim_start_matches(['=', '>', ' ', '\t', '"', '\'', '`']))
            else {
                continue;
            };
            let value: String = rest
                .chars()
                .take_while(|c| !c.is_whitespace() && !"\"'`,;)]}".contains(*c))
                .collect();
            if value.len() >= 16
                && value.chars().any(|c| c.is_ascii_digit())
                && value.chars().any(|c| c.is_ascii_alphabetic())
                && !value.contains(['<', '>', '{', '$', '…', '*'])
                && !value.contains("...")
                && entropy(&value) >= 3.5
            {
                return true;
            }
        }
    }
    false
}

/// The first thing in `text` that looks like a credential.
pub fn find(text: &str) -> Option<Found> {
    for (i, line) in text.lines().enumerate() {
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            return Some(Found {
                line: i + 1,
                kind: "private key",
            });
        }
        let words = line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));
        for word in words {
            if let Some(kind) = kind_of(word) {
                return Some(Found { line: i + 1, kind });
            }
        }
        if assigned_secret(line) {
            return Some(Found {
                line: i + 1,
                kind: "secret assigned to a name like password, token or key",
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_found() {
        let rt = format!("token: rt_{}", "a1".repeat(32));
        let gh = format!("export GH_TOKEN={}", format_args!("ghp_{}", "x".repeat(36)));
        let aws = "key AKIAIOSFODNN7EXAMPLE here";
        let pem = "-----BEGIN RSA PRIVATE KEY-----";
        let ant = format!("`sk-ant-api03-{}`", "q".repeat(30));
        let google = format!("key=AIza{}", "SyD-9tSrke72PouQMnMX-a7eZSW0jkFMBWY");
        let stripe = format!("sk_live_{}", "51H8bTqLkdIwHu7ixa2X9n0vDpQ");
        let openai = format!("OPENAI_API_KEY=sk-proj-{}", "Ab3dEf6hIj9kLm2nOp5qRs8t");
        let aws_secret = "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let json = r#"{"password": "Tr0ub4dor&3-horse-9xQ"}"#;
        let env = "DATABASE_TOKEN=4f9c2e81a7b3d6059e1f";
        let secret = "secret: c2VjcmV0LXZhbHVlLTEyMzQ1Njc4OQ";
        for (text, kind) in [
            (rt.as_str(), "review-tools agent token"),
            (gh.as_str(), "GitHub token"),
            (aws, "AWS access key"),
            (pem, "private key"),
            (ant.as_str(), "Anthropic API key"),
            (google.as_str(), "Google API key"),
            (stripe.as_str(), "Stripe key"),
            (openai.as_str(), "OpenAI API key"),
            (
                aws_secret,
                "secret assigned to a name like password, token or key",
            ),
            (
                json,
                "secret assigned to a name like password, token or key",
            ),
            (env, "secret assigned to a name like password, token or key"),
            (
                secret,
                "secret assigned to a name like password, token or key",
            ),
        ] {
            let found = find(&format!("# Title\n\n{text}\n")).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(found, Found { line: 3, kind }, "{text}");
        }
    }

    #[test]
    fn prose_about_credentials_is_fine() {
        for text in [
            "The token is `rt_…`, stored as a hash.",
            "Set RT_TOKEN, or a network secret: `Authorization: Bearer <token>`.",
            "AKIA keys are long-lived; we use none.",
            "A ghp_ prefix marks classic GitHub tokens.",
            "-----BEGIN CERTIFICATE-----",
            "rt_api::doc::parse and rt_server::service",
            "password: <your password here>",
            "token = ${RT_TOKEN}",
            "API_KEY=changeme",
            "The secret: keep it in SSM, never in the template.",
            "Commit 1a6b325b60cea883b10adbe6595ea9f92e316bc9 fixed it.",
            "token_count: 1234567890123456",
            "sk-learn is a library; sk_live_ keys are Stripe's.",
        ] {
            assert_eq!(find(text), None, "{text}");
        }
    }
}
