---
name: clickup-cli
description: Use when a task needs ClickUp data or actions from the command line — reading or writing tasks, lists, spaces, comments, docs, chat, time tracking, custom fields, or any ClickUp API call. Covers picking the right subcommand, bootstrapping an account, recipes, and triaging errors by exit code.
---

# clickup-cli

Single-binary CLI for the ClickUp API. stdout is always exactly one JSON value;
errors are JSON on stderr. Add `--pretty` for humans.

## ID hierarchy

`workspace` (ClickUp calls it a "team") → `space` → `folder` (optional) → `list` → `task`.
Most commands take the id of the object they act on directly (a list id, a task id, ...);
team-scoped commands (`workspace *`, `task filter`, `field list` with no location, `view list`
with no location, all `doc`/`chat` v3 families) use the account's configured default workspace
unless overridden with the global `--workspace <id>`. Custom task ids (e.g. `DEV-123`) need
`--custom-id` on every task-addressed command — without it, ids are treated as ClickUp's own.

## Pick the right command

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
| A task's assigned members, or time spent in each status | `clickup-cli task members\|time-in-status` |
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
| Guests, audit logs, custom roles, legacy per-task time tracking, or anything else without a dedicated command | `clickup-cli raw <METHOD> <path> ...` |

## Bootstrap (once per machine)

```bash
export CLICKUP_CLI_TOKEN='<token>'   # or omit to be prompted interactively
clickup-cli account add work --workspace-id 9001   # --workspace-id only needed if the token sees more than one
clickup-cli account test                           # verifies the token end-to-end
```

First account added becomes the default; select others with `--account <alias>` or
`$CLICKUP_ACCOUNT`.

## Recipes

- **Find tasks assigned to someone across the workspace:**
  `clickup-cli task filter --assignee 75413867 --all`
- **Create a task with custom fields:**
  `clickup-cli task create 124 --name "Fix login" --data '{"custom_fields":[{"id":"FIELD_UUID","value":"urgent"}]}'`
- **Post a comment:** `clickup-cli comment create --task 868k5a5wa --text "Deployed."`
- **Move vs add-to-list:** `task move` relocates a task out of its current list;
  `list add-task` puts the *same* task into an additional list without removing it from the
  first — the latter needs the Tasks in Multiple Lists ClickApp enabled on the workspace.
- **Upload an attachment:** `clickup-cli task attach 868k5a5wa ./report.pdf`
- **Log time against a task:**
  `clickup-cli time create --task 868k5a5wa --data '{"start":1735689600000,"duration":3600000}'`
- **Append to a doc page:**
  `clickup-cli doc page update doc1 page1 --content @notes.md --mode append`
- **Read a chat channel:** `clickup-cli chat messages chan1 --all`

## Errors: exit code → action

| Exit | kind | Action |
| --- | --- | --- |
| 0 | success | — |
| 1 | api | Read `message` (and `ecode` if present) from the JSON on stderr; fix the request. |
| 2 | usage | Bad flags/arguments — fix the invocation. |
| 3 | auth | Token rejected or missing (401 only — see below). Rerun `clickup-cli account add`. |
| 4 | network | Transport failure after retries already happened — the failure is real, not a rate limit to retry again. |

A 429 that survives all retry attempts still surfaces as exit 1 (`kind: "api"`, `status: 429`) —
the retry loop only produces exit 4 for a *transport*-level failure (connection refused, DNS,
timeout) after retries, never for an HTTP response ClickUp actually sent back. And only a 401
maps to exit 3/`auth`; a 403 (forbidden) surfaces as exit 1/`api` like any other HTTP error —
a deliberate deviation from the spec's "401/403 → auth" wording, since a 403 usually means the
token is valid but lacks permission for that specific resource, not that the token itself is
bad.

## Gotchas

- stdout is ALWAYS one JSON value; parse it, never scrape stderr.
- Dates and durations are epoch MILLISECONDS, not seconds.
- Task writes take `markdown_content`; task reads need `--markdown` to get markdown back instead
  of plain text/HTML.
- `hasMore:true` in any merged `--all` envelope means the data is PARTIAL (hit the page cap) —
  treat it as incomplete, not as "no more pages".
- Comment listing is newest-first, 25 per page.
- A NEGATIVE `duration` on a time entry (in `list`/`get`/`current` responses) means that timer
  is currently running.
- Rate limit is 100 requests/min on most plans; the CLI retries 429s automatically — a 429 that
  still comes back after retries surfaces as exit 1/`api` (see the exit code table above), not
  exit 4.
- `field set --value` accepts a bare word as a plain string; no need to double-quote simple
  values like `urgent`.
- Tag and reaction names with spaces or reserved characters are percent-encoded into the URL path
  automatically (`%20`, not `+`).
- `list add-task`/`remove-task` need the Tasks in Multiple Lists ClickApp enabled.
- Custom task ids (e.g. `DEV-123`) need `--custom-id` on task-addressed commands.
- `task filter` and every v3 family (`doc`, `chat`, `task move`, `task attachments`) resolve
  against a workspace id — the account's default, or the global `--workspace` override.
- Guests, audit logs, custom roles, and legacy per-task time tracking are reachable only via
  `raw`, not a typed command.
- Deleting is permanent — the API offers no trash/restore.
