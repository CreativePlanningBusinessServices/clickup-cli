# clickup-cli

A single-binary Rust CLI wrapping the ClickUp API for AI agents and scripting: workspaces,
spaces, folders, lists, tasks, comments, checklists, custom fields, tags, time tracking, views,
goals, docs, chat, users, groups, webhooks, and templates. stdout is always exactly one JSON
value (errors are JSON on stderr too), so every command is pipeable to `jq` without scraping
human-readable text. Sibling of [`intacct-cli`](../intacct-cli) and modeled on
[`netsuite-cli`](https://github.com/CreativePlanningBusinessServices/netsuite-cli).

## Install

**Release zip:** download the archive for your platform from the
[latest release](../../releases/latest), unzip, and put `clickup-cli` on your `PATH`. Prebuilt
for `aarch64-apple-darwin`, `x86_64-apple-darwin`, and `x86_64-pc-windows-msvc`. This is a
private repo, so downloading a release asset (and running `clickup-cli update`) needs a GitHub
token with `repo` scope — set `GITHUB_TOKEN`, or `gh auth token` if you have the `gh` CLI
authenticated.

**From source:**

```bash
cargo install --path .
```

Linux has no OS-keyring backend wired up (`keyring` falls back to an in-memory mock, so stored
credentials do not persist across runs) — macOS and Windows are the supported platforms; Linux
is CI-only, never a release target.

## Auth model

ClickUp personal API tokens (`pk_...`) never expire and are sent as-is in a raw `Authorization`
header — no `Bearer` prefix, no OAuth flow, no refresh. `account add` stores one token per
account alias in the OS keychain (via `keyring`).

## Bootstrap (once per machine)

```bash
export CLICKUP_CLI_TOKEN='<token>'   # or omit to be prompted interactively
clickup-cli account add work --workspace-id 9001   # --workspace-id only needed if the token sees more than one
clickup-cli account test                            # verifies the token end-to-end (GET /api/v2/user)
```

The first account added becomes the default; select others with `--account <alias>` or
`$CLICKUP_ACCOUNT`.

## Commands

`workspace` (ClickUp calls it a "team") → `space` → `folder` (optional) → `list` → `task`. Most
commands take the id of the object they act on directly; team-scoped commands (`workspace *`,
`task filter`, unlocated `field`/`view` lists, all `doc`/`chat`) use the account's configured
default workspace unless overridden with the global `--workspace <id>`.

| You need | Command |
| --- | --- |
| Workspaces the token can see, seats, plan, shared hierarchy, task types | `clickup-cli workspace list\|seats\|plan\|shared\|task-types` |
| Spaces in a workspace | `clickup-cli space list\|get\|create\|update\|delete` |
| Folders in a space | `clickup-cli folder list\|get\|create\|update\|delete\|move\|from-template` |
| Lists in a folder/space, and list membership | `clickup-cli list list\|get\|create\|update\|delete\|members\|from-template\|add-task\|remove-task` |
| Task CRUD, list-scoped listing | `clickup-cli task create\|get\|update\|delete\|list` |
| Tasks across the whole workspace | `clickup-cli task filter --assignee ... --all` |
| Move/merge a task, or seed one from a template | `clickup-cli task move\|merge\|from-template` |
| Tag/dependency/link a task, attach a file | `clickup-cli task tag\|dependency\|link\|attach\|attachments` |
| Comments on a task, list, or view | `clickup-cli comment list\|create\|update\|delete\|replies\|reply` |
| Checklists and their items | `clickup-cli checklist create\|update\|delete`, `checklist item add\|update\|remove` |
| Custom fields and their per-task values | `clickup-cli field list\|set\|remove` |
| Space-level tags (task tagging is `task tag`) | `clickup-cli tag list\|create\|update\|delete` |
| Time entries, the running timer, time-entry tags | `clickup-cli time list\|get\|create\|update\|delete\|history\|current\|start\|stop`, `time tag ...` |
| Views and the tasks a view resolves to | `clickup-cli view list\|get\|create\|update\|delete\|tasks` |
| Goals and their key results | `clickup-cli goal list\|get\|create\|update\|delete`, `goal key-result ...` |
| Docs (v3) and their pages | `clickup-cli doc search\|create\|get\|page-listing\|pages\|page` |
| Chat (v3): channels, messages, replies, reactions | `clickup-cli chat channels\|channel\|messages\|send\|message\|replies\|reply\|reactions\|react\|unreact` |
| Workspace members, user groups | `clickup-cli user get\|invite\|edit\|remove`, `clickup-cli group list\|create\|update\|delete` |
| Webhook subscriptions | `clickup-cli webhook list\|create\|update\|delete` |
| Task/list/folder templates | `clickup-cli template list <task\|list\|folder>` |
| Guests, audit logs, custom roles, legacy per-task time tracking, or anything else without a dedicated command | `clickup-cli raw <METHOD> <path> ...`, e.g. `clickup-cli raw GET /api/v2/team/9001/guest/42` |

See [`skills/clickup-cli/SKILL.md`](skills/clickup-cli/SKILL.md) (installed via `clickup-cli
skill install`, see below) for the full agent-facing usage guide — recipes, gotchas, the ID
hierarchy — and
[`docs/superpowers/specs/2026-08-24-clickup-cli-design.html`](docs/superpowers/specs/2026-08-24-clickup-cli-design.html)
for the design spec. `clickup-cli <command> --help` covers flags for any one subcommand.

## Output contract

stdout is always exactly one JSON value on success; nothing else is ever written there. On
failure, stdout is empty and stderr carries one JSON error envelope (`{"kind", "message", ...}`).
Add `--pretty` for human-readable indented JSON.

| Exit | kind | Meaning |
| --- | --- | --- |
| 0 | — | Success. |
| 1 | api | ClickUp returned an error. Read `message` (and `ecode` if present) from the JSON on stderr. |
| 2 | usage | Bad flags/arguments — fix the invocation. |
| 3 | auth | Token missing or rejected. Run `clickup-cli account test`; re-add the account if the token was revoked. |
| 4 | network | Transport failure after retries already happened (the CLI retries 429s automatically) — the failure is real, not a rate limit to retry again. |

### Pagination

List-shaped commands take `--page <n>` (default 0, one page at a time) or `--all` to fetch and
merge every page into `{"items": [...], "count": n, "hasMore": bool}`. `hasMore: true` in an
`--all` envelope means the merge hit its page cap and the result is **partial**, not that ClickUp
has more pages sitting unfetched behind a `hasMore: false`-shaped cursor — treat it as incomplete
and narrow the query (filters, date windows) rather than assuming one more `--all` run would
finish it.

## Environment variables

| Variable | Used for |
| --- | --- |
| `CLICKUP_ACCOUNT` | Default account alias, overridden by `--account`. |
| `CLICKUP_CLI_TOKEN` | Personal API token for `account add`, in place of the interactive prompt. |
| `CLICKUP_API_BASE` | Overrides the ClickUp API base URL (sandboxes, proxies, tests). Defaults to `https://api.clickup.com`. |
| `GITHUB_TOKEN` | Auth for `clickup-cli update` and manual release-asset downloads — this repo is private. |
| `CLAUDE_CONFIG_DIR` | Where `clickup-cli skill install` writes `SKILL.md` when `--dir` isn't given, in place of `~/.claude`. |

## Skill install / update

`clickup-cli skill install [--dir <path>]` writes the embedded `SKILL.md` to (in precedence
order) `--dir`, `$CLAUDE_CONFIG_DIR/skills/clickup-cli`, or `~/.claude/skills/clickup-cli`. It
never fails — it skips (reporting `installed: false` with a `reason`) when:

- the target is a symlink (or lives under one) — the skill tracks its source repo via git instead;
- the target already holds byte-identical content — nothing to do;
- there's nowhere to put it — no `--dir`, no `$CLAUDE_CONFIG_DIR`, and either no home directory
  or `~/.claude` doesn't exist yet (the bare home fallback won't create it as a side effect).

`clickup-cli update [--check] [--no-skill]` pulls the latest GitHub release, then re-execs the
*new* binary's `skill install` so the embedded skill on disk matches the binary that just
landed — pass `--no-skill` to skip that. **Version-skew caveat:** `skill install` always writes
whatever `SKILL.md` is embedded in the binary you invoke, never the latest one on GitHub — after
`update --no-skill`, or after `cargo install --path .` from an older checkout, the installed
skill can describe an older command surface than the binary you're actually running.

## Development

```bash
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

### Live smoke tests

`tests/live_smoke.rs` exercises a configured account against the real ClickUp API. It's excluded
from `cargo test` by default (`#[ignore]`) and from CI (CI compiles it but never runs it); run it
manually against an account already added via `account add`:

```bash
CLICKUP_LIVE_ALIAS=<alias> cargo test --test live_smoke -- --ignored --nocapture
```

### Release procedure

Bump the version, commit, then tag and push to trigger the release workflow, which builds, zips,
and publishes binaries for macOS (arm64/x86_64) and Windows (x86_64):

```bash
# bump `version` in Cargo.toml to X.Y.Z, then:
git commit -am "chore: bump version to X.Y.Z"
git tag vX.Y.Z && git push --tags
```

## Task

ClickUp: [868k5a5wa](https://app.clickup.com/t/868k5a5wa).
