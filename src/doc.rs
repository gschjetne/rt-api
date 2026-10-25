//! The review document format: Markdown with optional YAML front matter and
//! fenced `decision` blocks.
//!
//! ````markdown
//! ---
//! work_package: wp-03
//! title: "Milestone 3: offline sync"
//! ---
//!
//! ## Decision: Conflict resolution strategy
//! ```decision
//! id: conflict-strategy
//! kind: choice
//! options:
//!   - last-writer-wins (simplest, loses edits)
//!   - per-field merge (what the prototype does)
//! recommend: per-field merge
//! ```
//! Support material.
//! ````
//!
//! A decision owns the heading right before its block (the title, minus a `Decision:`
//! prefix) and the text up to the next heading of the same or a higher level. That text,
//! without the block, is the decision's body. Numbers are not written in the document: the
//! session assigns them on first appearance.

use std::fmt;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value as Yaml;

use crate::model::DecisionKind;

/// A parsed review document.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub front: FrontMatter,
    pub decisions: Vec<DecisionSpec>,
    /// The document in order, with each decision's section replaced by a reference to it.
    pub segments: Vec<Segment>,
    /// Relative link and image targets, candidates for attachments.
    pub local_refs: Vec<String>,
    /// Things that parsed but are probably mistakes.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub work_package: Option<String>,
    pub project: Option<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Segment {
    Markdown { text: String },
    Decision { id: String },
}

/// One decision as the document states it.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionSpec {
    pub id: String,
    pub content: DecisionContent,
    /// Set when the document withdraws the decision: `withdrawn: <reason>`.
    pub withdrawn: Option<String>,
    /// 1-based line of the decision block.
    pub line: usize,
}

/// The part of a decision that is versioned: changing any of it is a revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DecisionContent {
    pub kind: DecisionKind,
    /// For `choice`: whether several options may be chosen.
    #[serde(default)]
    pub multi: bool,
    pub title: String,
    /// For `choice`: the options, in order.
    #[serde(default)]
    pub options: Vec<String>,
    /// The recommended answer: options for `choice`, `go` or `no-go` for `go-nogo`, a
    /// suggested text for `text`.
    #[serde(default)]
    pub recommend: Vec<String>,
    /// Markdown support material.
    #[serde(default)]
    pub body: String,
}

impl DecisionContent {
    /// Names of the fields that differ, for reporting a revision.
    pub fn changed_fields(&self, other: &DecisionContent) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if self.kind != other.kind || self.multi != other.multi {
            fields.push("kind");
        }
        if self.title != other.title {
            fields.push("title");
        }
        if self.options != other.options {
            fields.push("options");
        }
        if self.recommend != other.recommend {
            fields.push("recommendation");
        }
        if normalize_body(&self.body) != normalize_body(&other.body) {
            fields.push("text");
        }
        fields
    }
}

/// Whitespace differences in a body are not revisions.
fn normalize_body(body: &str) -> String {
    body.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line, 0 when the error concerns the whole document.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

/// Every problem found in a document, so an author can fix them in one pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseErrors(pub Vec<ParseError>);

impl fmt::Display for ParseErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseErrors {}

/// Whether a string is a valid decision id: lowercase letters, digits, `-`, `_` and `.`,
/// starting with a letter or digit, at most 64 characters.
pub fn valid_decision_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c))
}

struct Heading {
    level: usize,
    start: usize,
    end: usize,
    text: String,
}

struct Fence {
    start: usize,
    end: usize,
    content: String,
}

pub fn parse(source: &str) -> Result<Document, ParseErrors> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let (front, body_start) = split_front_matter(source, &mut errors, &mut warnings);
    let body = &source[body_start..];
    let line_of = |offset: usize| source[..body_start + offset].matches('\n').count() + 1;

    let mut headings = Vec::new();
    let mut fences = Vec::new();
    let mut local_refs = Vec::new();
    let mut heading: Option<Heading> = None;
    let mut fence: Option<Fence> = None;
    for (event, range) in
        Parser::new_ext(body, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH)
            .into_offset_iter()
    {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(Heading {
                    level: level as usize,
                    start: range.start,
                    end: range.end,
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Heading(_)) => headings.extend(heading.take()),
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                if info.split_whitespace().next() == Some("decision") =>
            {
                fence = Some(Fence {
                    start: range.start,
                    end: range.end,
                    content: String::new(),
                });
            }
            Event::End(TagEnd::CodeBlock) => fences.extend(fence.take()),
            Event::Text(text) | Event::Code(text) => {
                if let Some(h) = heading.as_mut() {
                    h.text.push_str(&text);
                } else if let Some(f) = fence.as_mut() {
                    f.content.push_str(&text);
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                if let Some(path) = local_path(&dest_url)
                    && !local_refs.contains(&path)
                {
                    local_refs.push(path);
                }
            }
            _ => {}
        }
    }

    // Each fence's section: from its owning heading (the last heading after the previous
    // fence) to the next heading of the same or a higher level, or the next section.
    struct Section {
        start: usize,
        end: usize,
        owner: Option<usize>,
    }
    let mut sections: Vec<Section> = Vec::new();
    for (i, f) in fences.iter().enumerate() {
        let previous_end = if i == 0 { 0 } else { fences[i - 1].end };
        let owner = headings
            .iter()
            .rposition(|h| h.start < f.start && h.start >= previous_end);
        // A heading is not owned twice: if the previous section ends after it, no owner.
        let owner = owner.filter(|&h| sections.last().is_none_or(|s| s.end <= headings[h].start));
        let start = owner.map_or(f.start, |h| headings[h].start);
        let end = headings
            .iter()
            .find(|h| h.start >= f.end && owner.is_none_or(|o| h.level <= headings[o].level))
            .map_or(body.len(), |h| h.start);
        sections.push(Section { start, end, owner });
    }
    // A later section may begin before the computed end of an earlier one (a deeper heading
    // owning the next decision): cut the earlier one short.
    for i in 0..sections.len().saturating_sub(1) {
        let next_start = sections[i + 1].start;
        if sections[i].end > next_start {
            sections[i].end = next_start;
        }
    }

    let mut decisions: Vec<DecisionSpec> = Vec::new();
    let mut segments = Vec::new();
    let mut cursor = 0;
    for (f, s) in fences.iter().zip(&sections) {
        let line = line_of(f.start);
        push_markdown(&mut segments, &body[cursor..s.start]);
        cursor = s.end;

        let mut text = String::new();
        let mut title = None;
        if let Some(o) = s.owner {
            let h = &headings[o];
            title = Some(strip_decision_prefix(h.text.trim()));
            text.push_str(&body[h.end..f.start]);
        }
        text.push_str(&body[f.end..s.end]);

        match decision_from_block(
            &f.content,
            title,
            text.trim().to_owned(),
            line,
            &mut warnings,
        ) {
            Ok(spec) => {
                if decisions.iter().any(|d| d.id == spec.id) {
                    errors.push(ParseError {
                        line,
                        message: format!("decision id `{}` is used twice", spec.id),
                    });
                } else {
                    segments.push(Segment::Decision {
                        id: spec.id.clone(),
                    });
                    decisions.push(spec);
                }
            }
            Err(message) => errors.push(ParseError { line, message }),
        }
    }
    push_markdown(&mut segments, &body[cursor..]);

    let mut front = front;
    if front.title.is_none() {
        // Fall back to the first level-1 heading that is not a decision's.
        front.title = headings
            .iter()
            .enumerate()
            .find(|(i, h)| h.level == 1 && !sections.iter().any(|s| s.owner == Some(*i)))
            .map(|(_, h)| h.text.trim().to_owned());
    }

    if errors.is_empty() {
        Ok(Document {
            front,
            decisions,
            segments,
            local_refs,
            warnings,
        })
    } else {
        Err(ParseErrors(errors))
    }
}

fn push_markdown(segments: &mut Vec<Segment>, text: &str) {
    if !text.trim().is_empty() {
        segments.push(Segment::Markdown {
            text: text.trim().to_owned() + "\n",
        });
    }
}

fn split_front_matter(
    source: &str,
    errors: &mut Vec<ParseError>,
    warnings: &mut Vec<String>,
) -> (FrontMatter, usize) {
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return (FrontMatter::default(), 0);
    };
    let offset = source.len() - rest.len();
    let mut pos = 0;
    let close = loop {
        let Some(line_end) = rest[pos..].find('\n').map(|i| pos + i + 1) else {
            // A closing line without a trailing newline ends the document.
            if rest[pos..].trim_end() == "---" {
                break Some((pos, rest.len()));
            }
            break None;
        };
        if rest[pos..line_end].trim_end() == "---" {
            break Some((pos, line_end));
        }
        pos = line_end;
    };
    let Some((yaml_end, body_start)) = close else {
        errors.push(ParseError {
            line: 1,
            message: "front matter starts with `---` but is never closed".into(),
        });
        return (FrontMatter::default(), 0);
    };
    let yaml = &rest[..yaml_end];
    let mut front = FrontMatter::default();
    match serde_yaml_ng::from_str::<Yaml>(yaml) {
        Ok(Yaml::Mapping(map)) => {
            for (key, value) in map {
                let key = scalar(&key).unwrap_or_default();
                let target = match key.as_str() {
                    "title" => &mut front.title,
                    "work_package" | "work-package" => &mut front.work_package,
                    "project" => &mut front.project,
                    "preview" => &mut front.preview,
                    _ => {
                        warnings.push(format!("front matter: unknown key `{key}` ignored"));
                        continue;
                    }
                };
                match scalar(&value) {
                    Some(v) => *target = Some(v),
                    None => errors.push(ParseError {
                        line: 2,
                        message: format!("front matter: `{key}` must be a single value"),
                    }),
                }
            }
        }
        Ok(Yaml::Null) => {}
        Ok(_) => errors.push(ParseError {
            line: 2,
            message: "front matter must be a mapping of keys to values".into(),
        }),
        Err(e) => errors.push(ParseError {
            line: 1 + e.location().map_or(1, |l| l.line()),
            message: format!("front matter: {e}"),
        }),
    }
    (front, offset + body_start)
}

/// A YAML scalar as a string; `None` for sequences, mappings and null.
fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Bool(b) => Some(b.to_string()),
        Yaml::Number(n) => Some(n.to_string()),
        Yaml::Tagged(t) => scalar(&t.value),
        Yaml::Null | Yaml::Sequence(_) | Yaml::Mapping(_) => None,
    }
}

fn strip_decision_prefix(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("decision") {
        // "Decision: X", "Decision 4: X", "Decision - X"
        let rest_trimmed = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
        if let Some(after) = rest_trimmed
            .strip_prefix(':')
            .or_else(|| rest_trimmed.strip_prefix(" -"))
            .or_else(|| rest_trimmed.strip_prefix('-'))
        {
            let cut = title.len() - after.len();
            let stripped = title[cut..].trim();
            if !stripped.is_empty() {
                return stripped.to_owned();
            }
        }
    }
    title.to_owned()
}

fn decision_from_block(
    yaml: &str,
    heading_title: Option<String>,
    body: String,
    line: usize,
    warnings: &mut Vec<String>,
) -> Result<DecisionSpec, String> {
    let map = match serde_yaml_ng::from_str::<Yaml>(yaml) {
        Ok(Yaml::Mapping(map)) => map,
        Ok(_) => return Err("a decision block must be a mapping such as `id: my-decision`".into()),
        Err(e) => return Err(format!("decision block: {e}")),
    };
    let mut id = None;
    let mut kind = None;
    let mut multi = false;
    let mut title = None;
    let mut options = Vec::new();
    let mut recommend = Vec::new();
    let mut withdrawn = None;
    for (key, value) in &map {
        let key = scalar(key).unwrap_or_default();
        match key.as_str() {
            "id" => id = scalar(value),
            "kind" => kind = scalar(value),
            "multi" => {
                multi = match value {
                    Yaml::Bool(b) => *b,
                    _ => return Err("`multi` must be true or false".into()),
                }
            }
            "title" => title = scalar(value),
            "options" => {
                let Yaml::Sequence(items) = value else {
                    return Err("`options` must be a list".into());
                };
                for item in items {
                    match scalar(item) {
                        Some(s) if !s.trim().is_empty() => options.push(s.trim().to_owned()),
                        _ => {
                            return Err(
                                "every option must be a non-empty single line of text".into()
                            );
                        }
                    }
                }
            }
            "recommend" | "recommended" | "recommendation" => match value {
                Yaml::Sequence(items) => recommend = items.iter().filter_map(scalar).collect(),
                v => recommend = scalar(v).into_iter().collect(),
            },
            "withdrawn" => {
                withdrawn = match value {
                    Yaml::Bool(false) | Yaml::Null => None,
                    Yaml::Bool(true) => Some(String::new()),
                    v => scalar(v),
                }
            }
            other => warnings.push(format!(
                "line {line}: unknown key `{other}` in decision block ignored"
            )),
        }
    }

    let id = id.ok_or("decision block has no `id`")?;
    if !valid_decision_id(&id) {
        return Err(format!(
            "decision id `{id}` is not valid: use lowercase letters, digits, `-`, `_` and `.`"
        ));
    }
    let kind = match kind.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("go-nogo" | "go-no-go" | "gonogo" | "go/no-go") => DecisionKind::GoNogo,
        Some("choice" | "single-choice") => DecisionKind::Choice,
        Some("multi-choice" | "multiple-choice" | "multichoice") => {
            multi = true;
            DecisionKind::Choice
        }
        Some("text") => DecisionKind::Text,
        Some(other) => {
            return Err(format!(
                "decision `{id}`: unknown kind `{other}`; use go-nogo, choice or text"
            ));
        }
        None => {
            return Err(format!(
                "decision `{id}` has no `kind` (go-nogo, choice or text)"
            ));
        }
    };
    let title = title
        .or(heading_title)
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| {
            format!("decision `{id}` has no title: put a heading before the block or add `title:`")
        })?;

    match kind {
        DecisionKind::Choice => {
            if options.len() < 2 {
                return Err(format!(
                    "decision `{id}`: a choice needs at least two `options`"
                ));
            }
            if let Some(dup) = options
                .iter()
                .enumerate()
                .find(|(i, o)| options[..*i].contains(o))
            {
                return Err(format!("decision `{id}`: option `{}` appears twice", dup.1));
            }
            if recommend.len() > 1 && !multi {
                return Err(format!(
                    "decision `{id}`: only a `multi: true` choice can recommend several options"
                ));
            }
            recommend = recommend
                .iter()
                .map(|r| {
                    resolve_option(&options, r).ok_or_else(|| {
                        format!("decision `{id}`: recommendation `{r}` matches no option")
                    })
                })
                .collect::<Result<_, _>>()?;
        }
        DecisionKind::GoNogo => {
            if !options.is_empty() {
                return Err(format!(
                    "decision `{id}`: a go-nogo decision takes no `options`"
                ));
            }
            if multi {
                return Err(format!("decision `{id}`: `multi` only applies to a choice"));
            }
            recommend = recommend
                .iter()
                .map(|r| match r.to_ascii_lowercase().as_str() {
                    "go" | "yes" | "true" => Ok("go".to_owned()),
                    "no-go" | "nogo" | "no go" | "no" | "false" => Ok("no-go".to_owned()),
                    _ => Err(format!(
                        "decision `{id}`: recommend `go` or `no-go`, not `{r}`"
                    )),
                })
                .collect::<Result<_, _>>()?;
        }
        DecisionKind::Text => {
            if !options.is_empty() {
                return Err(format!(
                    "decision `{id}`: a text decision takes no `options`"
                ));
            }
        }
    }

    Ok(DecisionSpec {
        id,
        content: DecisionContent {
            kind,
            multi,
            title: title.trim().to_owned(),
            options,
            recommend,
            body,
        },
        withdrawn,
        line,
    })
}

/// The option a reference names: the exact text, a capital letter (`B` is the second
/// option), or a unique case-insensitive prefix of at least two characters. A letter is
/// tried before prefixes, so `A` never means "the option starting with a".
pub fn resolve_option(options: &[String], reference: &str) -> Option<String> {
    let r = reference.trim();
    if let Some(o) = options.iter().find(|o| o.as_str() == r) {
        return Some(o.clone());
    }
    let mut chars = r.chars();
    if let (Some(c @ 'A'..='Z'), None) = (chars.next(), chars.next()) {
        return options.get(c as usize - 'A' as usize).cloned();
    }
    if r.chars().count() < 2 {
        return None;
    }
    let lower = r.to_lowercase();
    let prefixed: Vec<_> = options
        .iter()
        .filter(|o| o.to_lowercase().starts_with(&lower))
        .collect();
    match prefixed.as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    }
}

/// A link target that names a file next to the document.
fn local_path(dest: &str) -> Option<String> {
    let dest = dest.split(['#', '?']).next().unwrap_or("");
    let path = dest.trim_start_matches("./");
    valid_attachment_path(path).then(|| path.to_owned())
}

/// Whether a path can name an attachment: relative, `/`-separated, no `..`, no scheme.
pub fn valid_attachment_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example from section 5 of the plan.
    const PLAN_EXAMPLE: &str = r#"---
work_package: wp-03
title: "Milestone 3: offline sync"
preview: https://wp-03.example.dev
---

## Context
What was built, what was learned, links to research.

## Decision: Conflict resolution strategy
```decision
id: conflict-strategy
kind: choice
options:
  - last-writer-wins (simplest, loses edits)
  - per-field merge (what the prototype does)
  - CRDT (needs a new dependency, see research below)
recommend: per-field merge
```
Support material: ![sync conflict](attachments/conflict.png), `research/crdt-eval.md`.

## Decision: Ship milestone 3?
```decision
id: ship-m3
kind: go-nogo
```
"#;

    #[test]
    fn plan_example() {
        let doc = parse(PLAN_EXAMPLE).unwrap();
        assert_eq!(doc.front.work_package.as_deref(), Some("wp-03"));
        assert_eq!(
            doc.front.title.as_deref(),
            Some("Milestone 3: offline sync")
        );
        assert_eq!(
            doc.front.preview.as_deref(),
            Some("https://wp-03.example.dev")
        );
        assert_eq!(doc.decisions.len(), 2);

        let d = &doc.decisions[0];
        assert_eq!(d.id, "conflict-strategy");
        assert_eq!(d.line, 11);
        assert_eq!(d.content.kind, DecisionKind::Choice);
        assert_eq!(d.content.title, "Conflict resolution strategy");
        assert_eq!(d.content.options.len(), 3);
        assert_eq!(
            d.content.recommend,
            ["per-field merge (what the prototype does)"]
        );
        assert_eq!(
            d.content.body,
            "Support material: ![sync conflict](attachments/conflict.png), `research/crdt-eval.md`."
        );

        let d = &doc.decisions[1];
        assert_eq!(d.id, "ship-m3");
        assert_eq!(d.content.kind, DecisionKind::GoNogo);
        assert_eq!(d.content.title, "Ship milestone 3?");
        assert_eq!(d.content.body, "");

        assert_eq!(
            doc.segments,
            [
                Segment::Markdown {
                    text: "## Context\nWhat was built, what was learned, links to research.\n"
                        .into()
                },
                Segment::Decision {
                    id: "conflict-strategy".into()
                },
                Segment::Decision {
                    id: "ship-m3".into()
                },
            ]
        );
        assert_eq!(doc.local_refs, ["attachments/conflict.png"]);
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    }

    #[test]
    fn no_front_matter_title_from_h1() {
        let doc = parse("# Review of things\n\nIntro.\n").unwrap();
        assert_eq!(doc.front.title.as_deref(), Some("Review of things"));
        assert!(doc.decisions.is_empty());
    }

    #[test]
    fn deeper_heading_ends_section_only_at_same_level() {
        let src = "## Decision: A\n```decision\nid: a\nkind: text\n```\nBody of A.\n\n### Detail\nStill A.\n\n## Other\nNot A.\n";
        let doc = parse(src).unwrap();
        assert_eq!(
            doc.decisions[0].content.body,
            "Body of A.\n\n### Detail\nStill A."
        );
        assert_eq!(
            doc.segments.last(),
            Some(&Segment::Markdown {
                text: "## Other\nNot A.\n".into()
            })
        );
    }

    #[test]
    fn nested_decision_headings_split_sections() {
        let src = "## Decisions\n\n### A\n```decision\nid: a\nkind: go-nogo\n```\nAbout A.\n### B\n```decision\nid: b\nkind: go-nogo\n```\nAbout B.\n";
        let doc = parse(src).unwrap();
        assert_eq!(doc.decisions[0].content.title, "A");
        assert_eq!(doc.decisions[0].content.body, "About A.");
        assert_eq!(doc.decisions[1].content.title, "B");
        assert_eq!(
            doc.segments[0],
            Segment::Markdown {
                text: "## Decisions\n".into()
            }
        );
    }

    #[test]
    fn text_between_heading_and_block_is_body() {
        let src = "## Pick a database\nWe need one.\n```decision\nid: db\nkind: choice\noptions: [redb, sqlite]\nrecommend: A\n```\nMore.\n";
        let d = &parse(src).unwrap().decisions[0];
        assert_eq!(d.content.title, "Pick a database");
        assert_eq!(d.content.body, "We need one.\n\nMore.");
        assert_eq!(d.content.recommend, ["redb"]);
    }

    #[test]
    fn title_field_and_no_heading() {
        let src =
            "Intro.\n\n```decision\nid: x\nkind: go-nogo\ntitle: Do it?\nrecommend: yes\n```\n";
        let d = &parse(src).unwrap().decisions[0];
        assert_eq!(d.content.title, "Do it?");
        assert_eq!(d.content.recommend, ["go"]);
    }

    #[test]
    fn multi_choice_alias() {
        let src = "## Which\n```decision\nid: w\nkind: multi-choice\noptions: [a, b, c]\nrecommend: [a, c]\n```\n";
        let d = &parse(src).unwrap().decisions[0];
        assert!(d.content.multi);
        assert_eq!(d.content.recommend, ["a", "c"]);
    }

    #[test]
    fn withdrawn_marker() {
        let src =
            "## Old\n```decision\nid: old\nkind: go-nogo\nwithdrawn: superseded by `new`\n```\n";
        let d = &parse(src).unwrap().decisions[0];
        assert_eq!(d.withdrawn.as_deref(), Some("superseded by `new`"));
    }

    #[test]
    fn errors_are_collected_with_lines() {
        let src = "## A\n```decision\nkind: go-nogo\n```\n\n## B\n```decision\nid: b\nkind: choice\noptions: [one]\n```\n\n## C\n```decision\nid: B!\nkind: go-nogo\n```\n```decision\nid: d\nkind: maybe\n```\n";
        let errs = parse(src).unwrap_err().0;
        let lines: Vec<_> = errs.iter().map(|e| e.line).collect();
        assert_eq!(lines, [2, 7, 14, 18], "{errs:?}");
        assert!(errs[0].message.contains("no `id`"));
        assert!(errs[1].message.contains("at least two"));
        assert!(errs[2].message.contains("not valid"));
        assert!(errs[3].message.contains("unknown kind"));
    }

    #[test]
    fn duplicate_ids_and_bad_recommendation() {
        let src = "## A\n```decision\nid: a\nkind: go-nogo\n```\n## A again\n```decision\nid: a\nkind: go-nogo\n```\n## C\n```decision\nid: c\nkind: choice\noptions: [x, y]\nrecommend: z\n```\n";
        let errs = parse(src).unwrap_err().0;
        assert!(errs[0].message.contains("used twice"));
        assert!(errs[1].message.contains("matches no option"));
    }

    #[test]
    fn unclosed_front_matter() {
        let errs = parse("---\ntitle: x\n\nbody\n").unwrap_err().0;
        assert!(errs[0].message.contains("never closed"));
    }

    #[test]
    fn unknown_keys_warn() {
        let doc = parse(
            "---\nowner: me\n---\n## A\n```decision\nid: a\nkind: go-nogo\nprio: high\n```\n",
        )
        .unwrap();
        assert_eq!(doc.warnings.len(), 2, "{:?}", doc.warnings);
    }

    #[test]
    fn local_refs_skip_urls_and_parent_paths() {
        let src = "[a](https://x.dev) [b](notes/b.md#part) ![c](./img/c.png) [d](../secret) [e](#anchor) [f](mailto:x@y) [g](/abs)\n";
        assert_eq!(parse(src).unwrap().local_refs, ["notes/b.md", "img/c.png"]);
    }

    #[test]
    fn option_references() {
        let options: Vec<String> = ["Defer it", "Add it now", "API Gateway"]
            .map(String::from)
            .into();
        let r = |s| resolve_option(&options, s);
        assert_eq!(
            r("A").as_deref(),
            Some("Defer it"),
            "a letter is not a prefix"
        );
        assert_eq!(r("B").as_deref(), Some("Add it now"));
        assert_eq!(r("D"), None, "out of range");
        assert_eq!(r("a"), None, "a single lowercase letter is neither");
        assert_eq!(r("add").as_deref(), Some("Add it now"));
        assert_eq!(r("api").as_deref(), Some("API Gateway"));
        assert_eq!(r("Ad"), Some("Add it now".to_owned()));
        assert_eq!(r("De").as_deref(), Some("Defer it"));
        let ambiguous: Vec<String> = ["Ab one", "Ab two"].map(String::from).into();
        assert_eq!(resolve_option(&ambiguous, "ab"), None);
        assert_eq!(
            resolve_option(&ambiguous, "Ab two").as_deref(),
            Some("Ab two")
        );
    }

    #[test]
    fn decision_prefix_variants() {
        assert_eq!(strip_decision_prefix("Decision: Ship it"), "Ship it");
        assert_eq!(strip_decision_prefix("Decision 4: Ship it"), "Ship it");
        assert_eq!(strip_decision_prefix("Decision - Ship it"), "Ship it");
        assert_eq!(
            strip_decision_prefix("Decisions to make"),
            "Decisions to make"
        );
        assert_eq!(strip_decision_prefix("Decision"), "Decision");
    }

    #[test]
    fn body_whitespace_is_not_a_revision() {
        let a =
            parse("## A\n```decision\nid: a\nkind: text\n```\nLine one.  \nLine two.\n").unwrap();
        let b =
            parse("## A\n```decision\nid: a\nkind: text\n```\n\nLine one.\nLine two.\n\n").unwrap();
        assert!(
            a.decisions[0]
                .content
                .changed_fields(&b.decisions[0].content)
                .is_empty()
        );
    }
}
