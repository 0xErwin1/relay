# Relay architecture

Relay is a keyboard-first, local-first HTTP client built on GPUI. It reads and writes Postman
v2.1 collections and environments, runs Postman `pm.*` scripts, and exposes the same engine to
AI agents through MCP. The workspace on disk is a directory of small text files, so the
collaboration path is git.

This document records the decisions made before the first line of product code. Each one
names the alternative it rejects and, where relevant, the DBFlux experience behind it.

## Decisions at a glance

| Area | Decision |
|------|----------|
| Source of truth | A workspace is a directory of TOML files, one file per request, folder, collection and environment. |
| App state | History, recent workspaces, cookies and UI state live in a per-user SQLite file, never in the workspace. |
| Secrets | Secret variable values live in the OS keyring; workspace files hold only the key and `secret = true`. |
| Engine | One `relay_engine` crate drives load, resolve, script, send and record. The UI, the MCP server and the CLI all call it. |
| MCP | `relay mcp` is a stdio server over `relay_engine`. It needs no shared database with the GUI because both read the same files. |
| Scripting | QuickJS through `rquickjs`, with a `pm` API shim and bundled Postman sandbox libraries. |
| Postman | Import and export of collection v2.1 and environment files. Unsupported fields are preserved, not dropped. |
| HTTP | `reqwest` with rustls, per-request clients cached by TLS and proxy configuration. |
| GPUI | Published `gpui-pre` and `gpui-component` crates, no vendored patches until a concrete need appears. |
| Crates | Ten crates in a strict one-way dependency order. Only `relay_ui` and `relay` link GPUI. |

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

Writes are atomic (temporary file plus rename) and carry the content hash the writer last
read. A mismatch means someone else changed the file (another window, `relay mcp`, a `git
pull`) and the write fails with a conflict instead of overwriting. The GUI watches the
workspace directory and reloads changed files.

## Crate map

```text
relay_core ──┬── relay_store ───┐
             ├── relay_postman ─┤
             ├── relay_http ────┼── relay_engine ──┬── relay_mcp ──┐
             └── relay_script ──┘                  └── relay_ui ───┴── relay (binary)
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
- Responses keep the raw bytes. Pretty-printing, JSON paths and previews are views over them.
- `relay_engine` owns a Tokio runtime. GPUI tasks await Tokio `JoinHandle`s, which are
  executor-agnostic futures, so no GPUI code runs on Tokio and no HTTP code runs on GPUI's
  executor.

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
- Every run has a memory limit and an interrupt-based time limit, so a `while (true)` loop in a
  shared collection cannot hang the app.

## Postman compatibility

- Collection v2.1 and environment JSON, import and export.
- Fields Relay does not model yet are kept in an `extensions` table on the entity and written
  back on export, so a round trip loses nothing.
- Import maps Postman items to the on-disk layout: folders to directories, request names to
  slugged file names (with a numeric suffix on collision), scripts to sidecar files.
- A fixture corpus of real exported collections drives round-trip tests: import, export and
  compare the JSON structurally.

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

`relay mcp --workspace <path>` runs an rmcp stdio server over `relay_engine`.

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

- Sidebar tree: workspace, collections, folders, requests, environments.
- Documents in tabs: request editor, collection and folder settings, environment editor,
  runner results.
- Request editor: method and URL bar, then tabs for params, headers, body, auth, scripts and
  settings.
- Response view: body (pretty, raw, preview), headers, cookies, timings, test results and
  script console.
- Keyboard-first: GPUI `KeyBinding`s with context predicates, a command palette, and a test
  that every action reachable by pointer is also bound to a key.
- Theming through semantic tokens mapped onto the gpui-component theme, with a test that
  rejects raw colors in components.

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
| No vendored GPUI patches up front. | Four patched vendored crates carry a refresh procedure each. Adopt a patch only for a concrete defect; the first candidate is the Wayland held-modifier fix in `gpui-pre-linux`. |
| A hard 1000-line limit on source files from day one, with no grandfathering. | DBFlux carries 2,000 to 3,900 line `lib.rs` and `mod.rs` files. |
| Persistence never depends on governance crates. | `dbflux_storage` depends on `dbflux_approval`. |
| Docs updated in the same change as the code they describe. | DBFlux still documents a removed hand-rolled MCP transport. |

## Roadmap

| Phase | Outcome |
|-------|---------|
| 0 Foundation | This document, a compiling workspace, dev shell, lint and CI. |
| 1 Model and store | `relay_core` types and templating, `relay_store` load and save with round-trip tests. |
| 2 HTTP | `relay_http` with basic, bearer and API key auth; `relay run` sends a request from the CLI. |
| 3 UI MVP | Sidebar, request editor, response view, environment switcher. |
| 4 Postman | v2.1 collection and environment import and export against a fixture corpus. |
| 5 Scripts | `pm.*` variables, tests and expectations, then `pm.sendRequest` and bundled libraries. |
| 6 MCP | Tools, resources, policy, approvals and audit. |
| 7 Depth | Client certificates, proxies, cookies, digest, OAuth 2, AWS v4, collection runner, native bundles. |

Phases 1 and 2 are the foundation everything else stands on; the UI starts once a request can
be loaded from disk and sent from the CLI.
