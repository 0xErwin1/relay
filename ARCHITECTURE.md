# Relay architecture

Relay is a local-first HTTP client built on GPUI. It reads and writes Postman
v2.1 collections and environments, runs Postman `pm.*` scripts, and exposes the same engine to
AI agents through MCP. The workspace on disk is a directory of small text files, so the
collaboration path is git.

This document records the decisions made before the first line of product code. Each one
names the alternative it rejects and, where relevant, the DBFlux experience behind it.

## Decisions at a glance

| Area | Decision |
|------|----------|
| Source of truth | A workspace is a directory of TOML files, one file per request, folder, collection and environment. |
| Saving | Edits are local drafts that survive restarts. `Ctrl+S` writes the workspace file. |
| Windows | One app instance with one workspace open at a time, as in DBFlux. Switching workspace replaces the window's content; each workspace keeps its own drafts. |
| Protocols | HTTP only until after v1. |
| App state | History, recent workspaces, cookies and UI state live in a per-user SQLite file, never in the workspace. |
| Secrets | Secret variable values live in the OS keyring; workspace files hold only the key and `secret = true`. |
| Engine | One `relay_engine` crate drives load, resolve, script, send and record. The UI, the MCP server and the CLI all call it. |
| MCP | `relay mcp` is a stdio server over `relay_engine`. It needs no shared database with the GUI because both read the same files. |
| Scripting | QuickJS through `rquickjs`, with a `pm` API shim and bundled Postman sandbox libraries. |
| Postman | Import and export of collection v2.1 and environment files. Unsupported fields are preserved, not dropped. |
| HTTP | `reqwest` with rustls, per-request clients cached by TLS and proxy configuration. |
| GPUI | Published `gpui-pre` 0.3.7, `gpui-component` 0.7.0 and `gpui-pre-platform` 0.3.7, the newest the UI automation bridge supports; every other dependency tracks its latest release. `gpui-pre` is patched from the `gpui-mcp` git repository at a pinned rev; nothing is vendored. |
| Crates | Ten crates in a strict one-way dependency order. Only `bolt`, `relay_ui` and `relay` link GPUI. |
| Design system | Bolt, copied from DBFlux, in the product-agnostic `bolt` crate. Relay supplies only its accent and its own tokens, so `bolt` can become a shared library without touching Relay's screens. |
| UI automation | The upstream `gpui-mcp` bridge is always installed. `relay mcp` exposes it as `ui_*` tools under the same policy, so agents declare one MCP server. |

## Domain model

```text
Workspace
├── globals            (variables, lowest precedence)
├── environments[]     (N per workspace)
└── collections[]      (N per workspace)
    ├── variables      (collection variables)
    ├── environments   (optional allowlist of workspace environments)
    ├── auth, scripts  (inherited by folders and requests)
    └── items[]
        ├── Folder     (auth, scripts, items[])
        └── Request    (method, url, params, headers, body, auth, scripts, settings, examples[])
```

- **Workspace** owns N collections and N environments.
- **Environment** belongs to the workspace. A collection may restrict which environments apply
  to it with an allowlist; an empty list means all of them. The UI only offers the allowed
  environments when a request of that collection is active.
- **Collection** owns N requests, optionally grouped in folders, plus collection variables,
  auth and scripts.
- **Request** covers any HTTP method, including custom verbs. Body modes follow Postman: `none`,
  `raw` (with a language hint), `urlencoded`, `formdata` (text and file parts), `binary` and
  `graphql`.
- **Auth** is an enum with `inherit` and `none` plus the Postman types: `basic`, `bearer`,
  `apikey`, `digest`, `oauth1`, `oauth2`, `awsv4`, `hawk`, `ntlm`, `jwt`, `edgegrid`, `asap`.
  `inherit` walks folder, then collection.
- **Example** is a saved request and response pair (Postman `response[]`).

Every entity carries a stable `id` (ULID). The file path is the human address; the id is what
history, MCP and Postman's `_postman_id` refer to. A copied file duplicates its id, so the loader
detects duplicates and assigns a fresh id to the later file, with a warning.

### Variable resolution

Precedence follows Postman, lowest to highest:

1. Workspace globals
2. Collection variables
3. Active environment
4. Iteration data (runner only)
5. Local variables (set by scripts for one execution)

`{{name}}` is resolved after pre-request scripts run, in the URL, params, headers, body, auth
fields and certificate paths. Dynamic variables (`{{$guid}}`, `{{$timestamp}}`,
`{{$randomInt}}` and the rest of Postman's list) resolve at the same point. An unresolved
variable stays literal and is reported in the response view, as Postman does.

### Execution pipeline

```text
load → collection pre-request → folder pre-request → request pre-request
     → resolve variables and auth → send → collection test → folder test → request test
     → record history
```

Each step reads and writes one `ExecutionContext` (variables by scope, request draft,
response, test results, console output). Scripts mutate the context, never the files; a
script that sets an environment value updates the current value, as in Postman.

## On-disk format

```text
billing/                              workspace root
├── relay.toml                        workspace manifest: id, name, format version, globals
├── environments/
│   ├── dev.env.toml
│   └── prod.env.toml
├── collections/
│   └── billing-api/
│       ├── collection.toml           id, name, variables, auth, environment allowlist, order
│       ├── collection.pre.js         optional collection pre-request script
│       ├── collection.test.js        optional collection test script
│       └── invoices/                 folder
│           ├── folder.toml           id, name, auth, order
│           ├── list-invoices.req.toml
│           ├── list-invoices.test.js
│           └── list-invoices.examples/
│               └── ok.toml
└── .relay/                           local, git-ignored: current values, per-user state
```

Rules that keep files mergeable:

- **One entity per file.** Two people editing two requests never touch the same file.
- **Order lives in the parent.** `order = ["list-invoices", "create-invoice"]` in
  `folder.toml` or `collection.toml`. Unlisted children append in name order. Reordering
  rewrites one file, not every sibling.
- **No volatile fields.** No `updated_at`, no last-response cache, no UI state in workspace
  files. Volatile data in shared files turns every save into a merge conflict.
- **Scripts are sidecar `.js` files.** Editors and linters work on them directly, and a diff
  of a script is a diff of code, not of an escaped string.
- **Stable serialization.** Keys are written in a fixed order and tables are only emitted when
  non-empty, so an unchanged entity serializes to the same bytes.
- **Shared versus current values.** An environment file holds the shared value. The current
  value a person or script sets lives under `.relay/`. Secret keys have no value in the file.
- **Format version.** `relay.toml` carries `format = 1`. A newer format refuses to load with a
  clear message rather than guessing.

- **Reserved request slugs.** A request cannot be named `collection` or `folder` on disk: its
  sidecars would be `collection.pre.js` or `folder.pre.js`, the parent's own scripts. Saving
  one fails; loading one skips it with a warning.
- **Stability scope.** Files Relay wrote reload and save to the same bytes. A file written by
  hand may be normalized once on its first save (string quoting, inline tables, unknown
  top-level keys moved under `[extensions]`); key order inside `[extensions]` and unmodeled
  auth types is preserved.

Writes are atomic (temporary file plus rename) and carry the content hash the writer last
read. A mismatch means someone else changed the file (another window, `relay mcp`, a `git
pull`) and the write fails with a conflict instead of overwriting. The check and the rename
are not locked together, so a write landing between them is lost; the window is one rename.
The GUI watches the workspace directory and reloads changed files.

### File schemas

Keys appear in the order shown. Defaults are never written: `enabled = true`,
`secret = false`, empty lists, `auth` of type `inherit`, and absent settings. Keys Relay does
not model are kept under `[extensions]` and written back. `relay_store`'s round-trip test
(`crates/relay_store/tests/round_trip.rs`) holds a fixture with every shape below.

`relay.toml`

```toml
id = "01JB7X3K9Q0000000000000001"
name = "Billing"
format = 1

[[globals]]
key = "host"
value = "api.test"

[[globals]]
key = "api_key"
secret = true            # a secret never has a value in the file
```

`environments/<slug>.env.toml`

```toml
id = "01JB7X3K9Q0000000000000002"
name = "Staging"

[[variables]]
key = "base_url"
value = "https://api.staging.test"
description = "staging gateway"

[[variables]]
key = "tenant_id"
value = "t_42"
enabled = false
```

`collections/<slug>/collection.toml` and `<folder>/folder.toml` (a folder has no
`environments` and no `variables`)

```toml
id = "01JB7X3K9Q0000000000000003"
name = "Billing API"
environments = ["01JB7X3K9Q0000000000000002"]   # allowlist; absent means all
order = ["invoices", "health"]

[auth]
type = "bearer"
token = "{{access_token}}"

[[variables]]
key = "version"
value = "v1"
```

`<slug>.req.toml`

```toml
id = "01JB7X3K9Q0000000000000004"
name = "Create invoice"
method = "POST"                     # any verb, stored uppercase
url = "{{base_url}}/v1/invoices"     # never holds the query string

[[params]]                           # the query string, in order
key = "expand"
value = "lines"

[[headers]]
key = "Idempotency-Key"
value = "{{$guid}}"

[auth]
type = "apikey"
key = "X-Api-Key"
value = "{{api_key}}"
location = "header"                  # or "query"

[body]
mode = "raw"
language = "json"
text = '{"currency": "usd"}'

[settings]
timeout_ms = 5000
follow_redirects = false
verify_tls = true
```

The other shapes:

| Table | Variants |
|-------|----------|
| `[auth]` | `type = "inherit"` (default, not written), `"none"`, `"basic"` (`username`, `password`), `"bearer"` (`token`), `"apikey"` (`key`, `value`, `location`), or any other type name with its fields kept as they are |
| `[body]` | `mode = "none"`, `"raw"` (`language`: `json`, `xml`, `text`, `html`, `javascript`; `text`), `"urlencoded"` (`[[body.fields]]` key and value rows), `"formdata"` (`[[body.parts]]` with `kind = "text"` and `value`, or `kind = "file"` and `path`), `"binary"` (`path`), `"graphql"` (`query`, `variables`) |
| key and value rows | `key`, `value`, optional `description`, `enabled = false` when disabled |

## Drafts and saving

Saving is explicit, as in Postman: an edit never touches the workspace file until `Ctrl+S`.

- An edited entity becomes a **draft**, stored in the per-user app state and keyed by
  workspace id and entity id. Drafts survive a restart and their tabs show them as unsaved.
  The app state is SQLite (`relay_engine::state`, `rusqlite`) at
  `$XDG_STATE_HOME/relay/state.sqlite3` (`$HOME/.local/state/relay/state.sqlite3` when the
  variable is unset), with an explicit, forward-only schema version so history tables can
  join later. A draft stores the whole entity document as TOML, so later editable surfaces
  need no new storage.
- **Sending** a request with a draft sends the draft. History records what was sent.
- **Saving** writes the entity file (atomic, hash-checked) and discards the draft. The
  engine entry points are `Engine::save_request`, which writes the request document back
  and returns the new file hash, and `Engine::request_base_hash`, which reports the hash
  the file had when the workspace was loaded.
- **Reverting** discards the draft.
- When the file changes underneath a draft (a `git pull`, an agent's write tool, another
  editor), the draft is kept and its tab shows that the saved version moved. Saving then
  fails with a conflict until the user picks a side.
- Agent write tools write files directly; they never create or read drafts. An agent and the
  user editing the same request meet at that conflict, not at a silent overwrite.

What the window does today: clicking the URL of an open request swaps it for a field
(`request-url-input`); typing stores a draft on every keystroke, Enter commits it to the
draft and Escape drops what was typed. In the Params and Headers grids, each row carries an
enable box (`request-<params|headers>-enabled-<n>`) whose click toggles the row's `enabled`
state in the same draft on the spot, the grid shows the draft's rows while a draft exists,
and clicking a row's key or value cell (`request-<table>-<key|value>-<n>`) swaps the cell
for a field (`request-cell-input`) seeded with the cell's text; Enter commits it to that
cell of the same draft, Escape drops what was typed, and clicking another cell commits the
open field first, like a spreadsheet. A value that references a secret is never seeded: the
field opens empty with a `Secret value` placeholder and an empty commit keeps the stored
reference. The row counts next to the Params and Headers sub-tabs read the draft, so a
toggle moves them. A drafted tab carries a tint dot, Save and Revert
wake up, `Ctrl+S` saves and reloads the workspace so the tree and later sends see the file,
and a moved file shows `File changed on disk` with the draft kept until Revert. Sending a
drafted request goes through `Engine::execute_draft`. Still read-only are the body, the
auth and the settings: they are the next slices, and each lands in the same draft record.
Undo, Redo and Split stay disabled.

Drafts stay out of the workspace directory so that a commit never picks one up.

## Crate map

```text
relay_core ──┬── relay_store ───┐
             ├── relay_postman ─┤
             ├── relay_http ────┼── relay_engine ──┬── relay_mcp ──┐
             └── relay_script ──┘                  └── relay_ui ───┴── relay (binary)
                                                         │
                                              bolt ──────┘   (design system, no Relay types)
```

| Crate | Owns | Must not |
|-------|------|----------|
| `relay_core` | Domain types, validation, variable scopes, `{{}}` templating, dynamic variables. Pure, no IO. | Touch the filesystem, network or GPUI. |
| `relay_store` | The on-disk format: load, save, atomic writes, conflict detection, file watching, native export bundles. | Know about HTTP or scripts. |
| `relay_postman` | Postman collection v2.1 and environment import and export. | Do IO beyond reading and writing JSON values. |
| `relay_http` | Building and sending requests: TLS, client certificates, CA bundles, proxies, cookies, redirects, timings, auth signing. | Resolve variables or run scripts. |
| `relay_script` | The QuickJS sandbox, the `pm` API, bundled libraries, time and memory limits. | Send requests directly; `pm.sendRequest` goes through a host callback. |
| `relay_engine` | The execution pipeline, collection runner, history and app state (SQLite), keyring secrets. The single API surface for all front ends. | Depend on GPUI or rmcp. |
| `relay_mcp` | The MCP server: tools, resources, agent policy, audit. | Duplicate engine logic. |
| `bolt` | Bolt, the design system shared with DBFlux: tokens, theme, generic components. | Know about Relay, HTTP or any product type. |
| `relay_ui` | Everything GPUI: theme, components, sidebar, documents, keymap. | Contain domain rules that the engine or core should own. |
| `relay` | The binary: GUI startup and the `mcp`, `run` and `import` subcommands. | Contain logic beyond wiring. |

`relay_ui` starts as one crate with modules. DBFlux split its UI into six crates and then had
to forward every feature flag through all of them; Relay splits only when compile times
measurably demand it, and then by one-way layers.

## HTTP engine

- `reqwest` with rustls, HTTP/1.1 and HTTP/2, gzip, brotli, deflate and zstd.
- Clients are cached by their TLS and proxy configuration, because a client certificate, a
  CA bundle or `danger_accept_invalid_certs` is per client in `reqwest`.
- Certificates follow Postman's model: a list of client certificates matched by host and port,
  plus an optional custom CA bundle and a per-request "verify TLS" toggle.
- Timings (DNS, connect, TLS, first byte, download) come from a custom resolver and a connector
  layer. If that proves insufficient, `relay_http` drops to `hyper-util` directly; the crate
  boundary keeps that change local.
- Known limits of the reqwest-based transport: repeated headers with the same name are grouped
  at the position of the first one (the order among their values is kept), and the reason
  phrase is the standard one for the status code rather than the server's own text. Both are
  local to `relay_http`; dropping to `hyper` fixes them if they ever matter.
- Responses keep the raw bytes. Pretty-printing, JSON paths and previews are views over them.
- `relay_engine` owns a Tokio runtime. GPUI tasks await Tokio `JoinHandle`s, which are
  executor-agnostic futures, so no GPUI code runs on Tokio and no HTTP code runs on GPUI's
  executor.

## Command line

`relay` with no arguments opens the window. `relay run` sends one request through
`relay_engine`, the same path the UI and MCP use:

```text
relay run <request> [--workspace <dir>] [--env <slug>] [--var <key>=<value>]... [-i|--include]
```

- `<request>` is a request id or its slug path, `<collection>/<folder>/.../<request>`.
- `--workspace` defaults to the current directory. `--env` selects an environment by slug;
  without it no environment applies. `--var` sets a local variable, the highest scope, and
  repeats.
- stdout is the response body, byte for byte. `--include` writes the status line
  `HTTP/<version> <code> <reason>` and the headers first, then a blank line.
- stderr has one `warning: <message>` line per workspace load warning, one
  `warning: unresolved variable {{<name>}}` line per unresolved variable, then the summary
  `<METHOD> <url> -> <code> <reason> in <ms> ms, <size>`, with the size in `B`, `KB` or `MB`
  (1024 based). Errors print `error: <message>`.

| Exit code | Meaning |
|-----------|---------|
| 0 | A response was received, whatever its status. |
| 1 | The request could not be executed: transport error, unsupported auth, environment not allowed for the collection, invalid GraphQL variables. |
| 2 | Usage error: bad arguments, unknown request or environment, workspace that fails to load. |

Secret variables have no value in workspace files, and the keyring does not exist yet, so a
secret resolves only when passed with `--var`. A `--var` value is visible in the process list
and the shell history; prefer reading it from the environment, such as
`--var token="$API_TOKEN"`.

The engine API behind it: `Engine::open(root)` loads the workspace; `Engine::prepare` selects
the request, checks the environment and resolves variables, auth, body and settings into a
`relay_http` request without IO (`relay_engine::prepare` is the same step as a pure function);
`Engine::execute` sends it as a task on the engine runtime and returns an `Execution` with its
`JoinHandle` and a cancellation token. Auth types other than `inherit`, `none`, `basic`,
`bearer` and `apikey` fail with `auth type <type> is not supported yet`.
`Engine::prepare_draft` prepares a draft request document in the place of the stored one,
keeping the stored request's parents (and with them the scopes and the environment
allowlist), and `Engine::execute_draft` sends it.

`Engine::inspect` is what a front end shows before sending, against the active environment or
none: the selected request with its scripts, the URL as written with the enabled params
appended after the separator `prepare` uses, the names `prepare` reports unresolved with that
environment, and `Inspection::spans`, which splits a text into literal runs and `{{name}}`
references marked as resolving (globals, collection, environment or dynamic variables) and as
secret (secret in the globals, the collection or an environment the collection allows). When
`prepare` fails with that environment (unknown environment, environment not allowed for the
collection, unsupported auth, invalid GraphQL variables), `problem` carries its error text,
`unresolved` is empty, and the spans still resolve with the environment when the collection
allows it and without one otherwise.

## Scripting

`rquickjs` embeds QuickJS, as DBFlux already does in `dbflux_js`.

- The `pm` object mirrors Postman: `pm.environment`, `pm.collectionVariables`, `pm.globals`,
  `pm.variables`, `pm.iterationData`, `pm.request`, `pm.response`, `pm.test`, `pm.expect`,
  `pm.cookies`, `pm.info`, `pm.sendRequest`, `pm.execution.setNextRequest` and
  `pm.execution.skipRequest`.
- The legacy globals (`tests[...]`, `responseBody`, `postman.setEnvironmentVariable`) are
  supported, because many shared collections still use them.
- `require()` serves the libraries the Postman sandbox ships (chai, lodash, moment,
  crypto-js, uuid, ajv, cheerio, xml2js, tv4, csv-parse), bundled as JavaScript assets and
  loaded on first use. All are MIT or similarly licensed; the bundle records each version and
  license.
- `pm.sendRequest` is asynchronous. The sandbox drives the QuickJS job queue and resolves the
  host promise when `relay_engine` returns the response.
- Every run is bounded three ways, so a script in a shared collection cannot hang the app or
  exhaust the machine: a 64 MiB QuickJS heap, a 5 s wall clock enforced by the interrupt
  handler, and an 8 MiB cap on what the host itself collects (console lines, test results and
  variable writes). The host cap exists because the interrupt is polled between bytecodes and
  cannot bound a run whose time sits in host calls — a loop of `console.log` with a large
  string grew past 3 GB before it existed — so every host call also checks the deadline and
  reports which limit stopped it.

What runs today (S7): before preparing a request, the engine runs the effective pre-request
script — the request's, else the innermost folder's that has one, else the collection's — with
`pm.request` as the stored document and the merged variables of the execution's scopes, and
applies its variable writes to those scopes for this run only, never to disk. After the
response it runs the effective test script the same way, with `pm.response`. Test results and
console entries travel beside the response (`Executed::scripts`); a failing test or a script
error — reported as an error console entry — never fails the request.

`relay run` prints the script results to stderr before its summary line, pre-request entries
first: one line per recorded test (`test pass <name>` or `test fail <name>: <message>`) and
one line per console entry (`console <level>: <message>`, level one of `log`, `info`, `warn`,
`error`). The exit code is unchanged by a failing test. Still later slices: `pm.sendRequest`,
`require()` of the bundled libraries, and the results panel and console in the app.

## Postman compatibility

- Collection v2.1 and environment JSON, import and export.
- Fields Relay does not model yet are kept in an `extensions` table on the entity and written
  back on export, so a round trip loses nothing.
- Import maps Postman items to the on-disk layout: folders to directories, request names to
  slugged file names (with a numeric suffix on collision), scripts to sidecar files.
- The engine writes the import: `relay_engine::import::postman` maps the documents and writes the
  collection, its folders and requests with their scripts and the environments through
  `relay_store`, creating the workspace manifest when the directory is empty and never
  overwriting a file that is already there.
- A fixture corpus of real exported collections drives round-trip tests: import, export and
  compare the JSON structurally.
- `relay import <collection.json> [--workspace <dir>] [--environment <file>]...` imports into a
  workspace directory: it creates the manifest when the directory has none, writes the
  collection and its environments, prints what it wrote, and refuses to run when a target file
  already exists, so an import never overwrites existing work.

## Export and import

| Unit | Native bundle | Postman |
|------|---------------|---------|
| Workspace | Yes | No (Postman has no portable workspace format) |
| Collection | Yes | Collection v2.1 |
| Folder | Yes | Collection v2.1 containing that folder |
| Request | Yes | Collection v2.1 containing that request |
| Environment | Yes | Environment JSON |

A native bundle is a single versioned file (`format_version`, `relay_version`, kind, payload).
Secrets are excluded by default. Including them requires either an age passphrase, which
encrypts only the secrets section, or an explicit plaintext flag. This is the DBFlux
portability model, which held up well.

Because the workspace is already a directory of text files, sharing a whole workspace through
git needs no export at all.

## MCP

`relay mcp [--workspace <path>] [--app-id <id>]` runs an rmcp stdio server. Today it serves
the `ui_*` tools of "UI automation"; the engine tools below are the plan.

What exists: `relay_mcp` reads the bridge endpoint descriptors under the runtime directory
and speaks the framed wire protocol to one window (`relay_mcp::bridge`), and serves thirteen
`ui_*` tools over stdio (`relay_mcp::server`). A session talks to the only live target, asks
for `ui_select_target` when several are running, and reports a target that died instead of
switching. Stdout carries only protocol frames; logs go to stderr. Text the bridge withholds
as redacted stays withheld, and a bridge failure is a tool error, never a panic.

Not yet: policy, approvals and audit. Every tool runs unpoliced, which adds no exposure the
bridge does not already have (see the trust model below), and the help text says so.

- **Tools**, grouped with one `#[tool_router]` per group: workspace (list, open, inspect),
  collection and folder (list, get, create, update, move, delete), request (get, create,
  update, move, delete, execute), environment (list, get, set variable), runner (run a
  collection or folder), history (list, get), and import and export.
- **Resources**: requests, collections and environments addressed by id, so an agent can read
  them without a tool call.
- **Secrets never leave the engine.** Tools return `{{token}}`, not the token. Execution
  resolves secrets inside the engine, and response output redacts any secret value it finds
  echoed back.
- **Policy** lives in the workspace manifest and the user's settings: whether agents may write
  files, which environments they may execute against, and a host allowlist. Each tool has an
  execution class (read, write, execute, destructive), mapped to allow, ask or deny.
- **Approvals.** With the GUI running, `ask` shows an approval prompt in the app over a local
  socket. Headless, `ask` resolves to deny. An agent can never approve its own call.
- **Audit.** Every write and execution is appended to a per-user audit log, with secrets
  redacted.

The GUI and `relay mcp` coordinate through the files themselves: atomic writes, hash-checked
saves and the file watcher. There is no second runtime that has to agree with the GUI about a
database, which was DBFlux's most expensive structural mistake.

## UI

What exists today: `relay [<dir>]` opens a workspace through `relay_engine` before the window
appears (a slow disk delays the window). The window is the Bolt shell of
`design/bolt-v2/01-main.html`: a title bar with the workspace and environment on the left,
the command search in the middle and the approvals bell on the right; the activity rail
(Collections, History, Environments, MCP approvals, Audit log, Settings); a sidebar island with
the collection tree (icons, indent guides, child counts, method labels in Bolt's semantic
colors) and a request and collection count; a document island; and a status bar that shows
the last send (`<METHOD> <path> · <status> · <time>`, or `sending` or `failed`) and
load warnings. In the tree the arrow keys move the selection, Enter or the chevron toggles a
folder, and a click or Enter on a request opens it in a document tab, or focuses its tab.
The environment button of the title bar opens a menu with `No environment` and the workspace
environments by name; choosing one makes it the window's active environment, and Escape or a
click outside closes the menu without a change. Icons are Lucide, from `gpui-kit-assets`,
installed by the binary as the application's asset source.

The document island shows the open requests as tabs, the path bar of the active one
(collection, folders, request), and the request read-only: an action strip (Save, Revert,
Undo, Redo, Split, disabled until editing exists), the request line (method, URL with the
enabled params, Send), and the Params, Headers, Body, Auth,
Scripts and Settings sub-tabs. What it shows comes from `Engine::inspect` with the window's
active environment, which lives in memory, starts as none and re-inspects every open request
when it changes, keeping its sub-tab: variables are styled as resolved or unresolved, the
number of unresolved ones is shown in the sub-tab bar, or the inspection `problem` in its
place (`request-problem`), and a value that references a secret, a password, a token, an API key value or
any field of an auth type the engine does not support is masked as `••••••••` on screen and
in the automation tree.

Send or Ctrl+Enter sends the active request with the active environment through
`Engine::execute`. The UI awaits the execution's `JoinHandle` on GPUI's executor with
`cx.spawn` and never calls `Engine::wait`, which blocks. While it is in flight, Send turns into
Cancel; Cancel or Escape cancels it through its token, sending again cancels and replaces it,
and closing the tab cancels it and drops its response. The response sits under the request:
the status chip colored by class, the time and size, Body and Headers sub-tabs, a
Pretty/Raw toggle (Pretty re-indents JSON keeping key order), the body split into lines once
off the UI thread and drawn with `uniform_list` so only visible lines render, an error banner
with Retry, and the unresolved variables the send used. Each open request keeps its last
response in memory only; history comes later.

Every region and tree row has a stable element id (`title-bar`, `sidebar`, `document`,
`status-bar`, `tree-row-<id>`, `doc-tab-<id>`, `request-tab-<name>`,
`request-<params|headers>-row-<n>`, `request-send`, `request-cancel`, `response-status`,
`response-meta`, `response-body-line-<n>`, `response-headers-row-<n>`, `response-error`,
`response-retry`, `status-last-request`), and text that automation must read carries a label role,
because the gpui-mcp bridge only reports text for nodes with a text role. Tree rows report
`selected` and `expanded`; tabs and sub-tabs report `selected`. On Wayland a window renders
and handles input only while its workspace is on screen, so automation needs the window
visible.

The plan:

- Sidebar tree: workspace, collections, folders, requests, environments.
- Documents in tabs: request editor, collection and folder settings, environment editor,
  runner results.
- Request editor: method and URL bar, then tabs for params, headers, body, auth, scripts and
  settings.
- Response view: body (pretty, raw, preview), headers, cookies, timings, test results and
  script console.
- Keyboard: GPUI `KeyBinding`s with context predicates, a command palette, and a test
  that every action reachable by pointer is also bound to a key.
- Theming through Bolt (see "Design system"), with a test that rejects raw colors and bare
  pixel values outside the token files.

## Design system

Relay and DBFlux are a family of apps: both use **Bolt** (islands, 45 degree cuts, Archivo and
JetBrains Mono, compact density) with one accent per product. `design/system.md` holds the
values; `design/bolt-v2/` holds the artboards of every view, built from DBFlux's own.

Bolt is copied from DBFlux for now. The goal is to make the later move to a shared library a
dependency swap, so the copy lives behind a boundary from the start:

- **`crates/bolt`** owns everything product-agnostic: color roles and washes, typography roles,
  spacing, cuts and chrome metrics, the chamfer shape, the theme mapping onto gpui-component,
  and generic components (island, button, input, tabs, tree row, table, keycap, status chip).
  It depends on GPUI and gpui-component only and never names Relay, HTTP or a Relay type.
- **Relay passes in what differs:** a `Brand` value (accent fill, hover, pressed, tint, app
  name, logo) and its own extra tokens (`method-*`, `draft`), defined in `relay_ui`.
- **`relay_ui` never reads gpui-component `Theme` fields or raw colors directly.** It asks
  `bolt` for a role (`text strong`, `panel`, `accent tint`) or uses a `bolt` component.
  A guardrail test rejects color literals and bare pixel values outside the token files, as
  DBFlux does.
- Extraction then means moving `crates/bolt` to its own repository, depending on it from
  both products, and deleting DBFlux's copy. Bolt values that change in the meantime are
  changed in both products or recorded as a divergence in `design/system.md`.

## UI automation

Every window installs the [`gpui-mcp`](https://github.com/themixednuts/gpui-mcp) bridge. It is
always on, not a development feature: agents drive the running app through the same `relay mcp`
server they use for collections and requests, so a client declares one MCP server, not two.

```text
agent ── stdio (MCP) ──> relay mcp ──┬── relay_engine          collections, requests, execution
                                     └── gpui-mcp-protocol ──> bridge in each Relay window
```

- `relay_mcp` speaks `gpui-mcp-protocol` to the bridge directly: endpoint discovery, the token
  handshake and operations such as reading the element tree, focusing, typing, pressing keys
  and clicking. There is no second process; `gpui-mcp-server` is upstream's own client and
  Relay does not ship it.
- UI tools are named `ui_*` and go through the same policy as every other tool. Reading the
  tree is class `read`; any input is class `execute`, because a click on Send executes a
  request just as `request_execute` does. Without that, the UI would be a way around the policy.
- With several Relay windows or processes, `ui_list_targets` lists them and `ui_select_target`
  chooses one; a single target is selected automatically.
- Screenshots need native window capture, which on Linux only works for X11 windows. They are
  a later addition through `gpui-mcp-capture`; the element tree and input work on Wayland.

Trust model, and what it costs:

- The bridge listens on an owner-only socket under `$XDG_RUNTIME_DIR/gpui-mcp/` and writes a
  descriptor with a random token. Any process running as the same user can read that token and
  drive the app or read what it renders, with no policy in between; only `relay mcp` enforces
  the policy. This is the same exposure the user's files and keyring already have to such a
  process, and it is the price of having automation always on.
- Secret values must never be rendered in clear text by default. Secret inputs are masked, and
  the bridge withholds masked and `frame_redacted` text from observers.
- The bridge, the patched `gpui-pre` and `gpui-mcp-protocol` all come from one `gpui-mcp`
  rev, set once in the root `Cargo.toml`. Bump it for all of them at once.
- `gpui-mcp` supports `gpui-pre` 0.3.5 to 0.3.7, which caps Relay at `gpui-component` 0.7.0
  until upstream supports a newer release.

`relay mcp` serves these tools itself: `ui_list_targets`, `ui_select_target`, `ui_ping`,
`ui_tree`, `ui_find`, `ui_element`, `ui_element_bounds`, `ui_wait_for_element`, `ui_click`,
`ui_type`, `ui_key`, `ui_focus` and `ui_scroll`. Relay reimplements discovery, descriptor
validation and the framing instead of depending on upstream's server, whose client is private,
so `scripts/gpui-mcp.sh` is no longer needed to drive the app.

The protocol crate gives the types; the transport, the size caps (1 MiB per request, 24 MiB
per response) and the 10 s I/O timeout are Relay's. A request carries the descriptor's token
and a fresh request id, and a response must match both the id and the protocol version.

## Lessons from DBFlux

| Keep | Evidence in DBFlux |
|------|--------------------|
| Capability seams so the UI never branches on a backend id. In Relay: auth types and body modes. | `AGENTS.md` "never branch on driver_id" |
| GPUI-native keymap with context predicates and a keyboard coverage test. | `dbflux_ui_base/src/keymap/mod.rs` |
| Semantic theme tokens with a guardrail test against raw colors. | `DESIGN.md`, `dbflux_components/src/theme.rs` |
| Versioned export bundle with age-encrypted secrets and an explicit plaintext flag. | `dbflux_portability/src/bundle.rs` |
| Execution classes, human-only approvals and redacted audit for agent actions. | `docs/MCP_AI_INTEGRATION.md` |
| Strict workspace lints (no `unwrap`, `expect`, `panic`, indexing outside tests). | root `Cargo.toml`, `clippy.toml` |
| Secrets in the OS keyring, references in storage. | `dbflux_core/Cargo.toml` (`keyring`) |
| A separate data directory per release channel, so a nightly migration cannot damage stable data. | `ARCHITECTURE.md` storage section |

| Change | Why |
|--------|-----|
| User content in text files instead of one SQLite file. | Collections must diff, merge and be reviewed in git. |
| MCP as a thin layer over the shared engine, in one crate. | DBFlux runs a second driver runtime in the MCP process, coordinates through a shared SQLite file and splits MCP across two overlapping crates. |
| One UI crate to start. | Six UI crates forced feature forwarding through every one of them and spread the keymap over three crates. |
| Vendor nothing. | DBFlux carries four patched vendored crates, each with its own refresh procedure. Relay uses published crates plus one git patch of `gpui-pre` from the `gpui-mcp` repository, which upstream maintains. |
| A hard 1000-line limit on source files from day one, with no grandfathering. | DBFlux carries 2,000 to 3,900 line `lib.rs` and `mod.rs` files. |
| Persistence never depends on governance crates. | `dbflux_storage` depends on `dbflux_approval`. |
| Docs updated in the same change as the code they describe. | DBFlux still documents a removed hand-rolled MCP transport. |

## Roadmap

| Phase | Outcome |
|-------|---------|
| 0 Foundation | This document, a compiling workspace, dev shell, lint and CI. |
| 1 Model and store | `relay_core` types and templating, `relay_store` load and save with round-trip tests. |
| 2 HTTP | `relay_http` with basic, bearer and API key auth; `relay run` sends a request from the CLI. |
| 3 UI MVP | Sidebar, request editor, response view, environment switcher, and `relay mcp` with the `ui_*` tools so agents verify the UI from here on. |
| 4 Postman | v2.1 collection and environment import and export against a fixture corpus. |
| 5 Scripts | `pm.*` variables, tests and expectations, then `pm.sendRequest` and bundled libraries. Required for v1. |
| 6 MCP | Workspace, collection, request and runner tools, resources, policy, approvals and audit. |
| 7 Depth | Client certificates, proxies, cookies, digest, OAuth 2, AWS v4, collection runner, native bundles. |

FEATURES.md assigns each feature to MVP, v1 or later. Phases 1 and 2 are the foundation everything else stands on; the UI starts once a request can
be loaded from disk and sent from the CLI.
