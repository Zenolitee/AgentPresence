# Architecture

The crate used to be a single ~2,860-line `src/main.rs`. It is now split into
small modules so a failure in one agent's detection code can't bury everything
else, and so each concern has one obvious place to look when debugging.

## Module map

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point only: declares modules, parses argv, dispatches subcommands (`status` / `debug` / `once` / `clear` / `run` / `tui`). |
| `config.rs` | `Config` + `DisplaySettings` structs, default values, `DEFAULT_CONFIG_JSON`, load/save of `%USERPROFILE%\.agent-presence\config.json`, per-agent display-setting resolution, raw field parsers. |
| `runtime.rs` | Long-running loops: `run_loop` (headless) and `run_tui_loop` (TUI + background presence thread). |
| `commands.rs` | One-shot subcommands: `print_status`, `print_debug`, `publish_once`. |
| `presence.rs` | Builds the Discord activity JSON: details line (flavor/active/idle rotation), state line assembly, first-seen timestamps, asset selection. |
| `discord.rs` | Everything Discord IPC: pipe discovery, `DiscordIpc` (handshake / SET_ACTIVITY / framing), client-id selection per agent, clearing stale presences, `json_escape`. |
| `harnesses/` | **One file per detected agent** (see below). |
| `detect.rs` | Process-list fallback detection (`Get-CimInstance` on Windows) and the shared process-scan cache. |
| `window.rs` | Foreground-window title reading (`user32` FFI) and path extraction from titles (drives "active project" detection). |
| `format.rs` | Pure formatting helpers: model names, token counts, plans, limits, cost, Discord state fitting, activity strings. |
| `fsutil.rs` | Filesystem utilities: newest-JSONL search, head/tail readers, mtime walks, git branch lookup, home dir, ISO-8601 stub. |
| `tui.rs` | The terminal UI (ratatui): live session panel + toggleable settings that persist via `config::save_config`. |

## The harness pattern (`harnesses/`)

Each agent gets exactly one file exporting a collector:

- `codex.rs` — reads `%USERPROFILE%\.codex\sessions` JSONL (head+tail parse)
- `claude.rs` — reads `~/.claude/sessions/*.json` + project JSONL + `settings.json`
- `pi.rs` — reads `~/.pi\agent\sessions` JSONL
- `omp.rs` — reads `~/.omp\agent\sessions` JSONL (reuses `pi::ParsedPiSession`)
- `hermes.rs` — reads `%LOCALAPPDATA%\hermes\sessions\sessions.json`
- `opencode.rs` — reads OpenCode desktop `.dat` files + `opencode.db` (SQLite)

`harnesses/mod.rs` owns the shared vocabulary (`AgentSession`, `AgentKind`,
`collect_session` dispatcher, `select_best_session` ranking) and calls each
harness's `collect_*_session(config)`.

**To add a new agent:** create `harnesses/<name>.rs` with
`pub fn collect_<name>_session(config: &Config) -> io::Result<Option<AgentSession>>`,
add an `AgentKind` variant, a `detect_<name>` config flag, wire it into
`collect_session`, and add client-id/asset entries in `config.rs` and
`presence.rs`. Nothing else needs to change.

## Data flow

```
main → runtime loop (poll every poll_seconds)
   → harnesses::collect_session
        ├─ per-harness session-file collection (staleness via stale_seconds)
        ├─ detect::detect_process_session (fallback, cached process scan)
        └─ select_best_session (active project > active > has path > newest)
   → presence::activity_payload  →  discord::DiscordIpc::set_activity
   (TUI mirrors state through a shared parking_lot::Mutex<AppState>)
```

## Locking rule (see `runtime.rs`)

The background presence thread must **never hold the shared `AppState` mutex
across `collect_session` or Discord pipe I/O**. The TUI's render loop takes the
same lock every frame; holding it across the multi-second process scan made
quit appear to hang. Config is cloned out of the lock instead.

## Process-scan cache

`detect::running_process_text` spawns PowerShell. One poll cycle used to spawn
it up to six times (every harness's liveness check); results are now cached for
one second, so a cycle costs one scan. `status` went from multiple seconds to
~0.5s.

## Known leftovers

- `format_omp_model` in `harnesses/omp.rs` is dead code (was never called in
  the original monolith either); kept as a compile warning.
- `parse_iso8601_utc` in `fsutil.rs` is a stub that always returns `None`
  (original behavior preserved), so Pi/OMP `started_at` falls back to file mtime.
- Build note: this machine uses the Rust GNU toolchain
  (`stable-x86_64-pc-windows-gnu`) with the winget WinLibs MinGW GCC as linker
  because MSVC Build Tools are not installed.
