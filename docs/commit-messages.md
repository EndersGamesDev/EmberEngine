# Commit messages

Commit subjects follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) so repository history remains machine-readable.

## Subject

Every subject has this form:

```text
<type>(<scope>)!?: <summary>
```

The `!` is optional and marks a breaking change.

`<type>` is one of `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `build`, `ci`, `perf`, `style`, or `revert`.

`<scope>` is a crate name with any leading `ember-` removed, a game id, a lab name, or one of `deploy`, `docs`, `tools`, `web`, or `workspace`.

`<summary>` uses the imperative mood, begins with a lower-case letter, has no trailing period, and keeps the complete subject to at most 72 columns.

Examples:

```text
feat(arena): add spectator controls
fix(arena-core)!: reject stale protocol frames
docs(workspace): explain release ownership
```

## Body and footer

Body paragraphs explain why the change is needed and state explicitly what was and was not verified.

When a change alters a protocol version, launcher schema, or deploy contract, add a `BREAKING CHANGE:` footer that describes the compatibility boundary and required migration.

## Merge commits

A merge commit uses the same `<type>(<scope>): <summary>` subject and describes the merged change as a whole; `merge(<scope>): <summary>` is not a Conventional Commit.

Put `Merge branch` details in the body when they are useful for tracing integration history.

## Why this is enforced

Machine-readable history feeds the changelog's pending section and the version-bump rule: every merge advances the patch grade. See `docs/versioning.md` for the complete version and release policy.
