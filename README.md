# rt-api

The types of the [review-tools](https://github.com/gschjetne/rt-cli) JSON API, and the
review document format: its parser, the differ between revisions, and the secret detector
that keeps credentials out of reviews. No I/O.

- `model`: requests and responses of `/api/v1`. With the `openapi` feature, every type
  also describes its OpenAPI schema.
- `doc`: a review document (Markdown with front matter and fenced `decision` blocks) into
  its decisions, attachments and links.
- `diff`: what changed between two revisions of a document, by decision id.
- `secrets`: credentials and keys in text, before it leaves the machine.
- `remote`: which repository on a code host a git remote names.

The client, `rt`, is in [rt-cli](https://github.com/gschjetne/rt-cli). Depend on a
release tag:

```toml
rt-api = { git = "https://github.com/gschjetne/rt-api", tag = "v0.1.0" }
```

`Project::repo_path` and `PrincipalKind::Local` belong to the local daemon, which is gone;
they go in 0.2.0.

MIT licensed.
