# AgentPresence - Transparency Report

This document explains exactly what AgentPresence does, what data it accesses, and how it works. Built for users who want to verify the app is safe before running it.

## What It Does

AgentPresence is a Discord Rich Presence client. It detects which AI coding agent (Codex, Pi, OpenCode, Oh My Pi) is running in your terminal and updates your Discord status to show which agent you're using.

## What It Accesses

### Files Read (Read-Only)

| Path | Purpose |
|------|---------|
| `%USERPROFILE%\.codex\sessions\**\*.jsonl` | Codex session files (project name, model, tokens, activity) |
| `%USERPROFILE%\.pi\agent\sessions\*.jsonl` | Pi session files (project name, model, tokens) |
| `%USERPROFILE%\.omp\agent\sessions\*.jsonl` | Oh My Pi session files (project name, model, tokens) |
| `%USERPROFILE%\.local\share\opencode\opencode.db` | OpenCode SQLite database (model info) |
| `%APPDATA%\ai.opencode.desktop\opencode.global.dat` | OpenCode global config (project path) |
| `%APPDATA%\ai.opencode.desktop\opencode.workspace.*.dat` | OpenCode workspace state (branch info) |
| `%USERPROFILE%\.agent-presence\config.json` | App configuration |
| `.git\HEAD` | Git branch detection (only when inside a git repo) |

### Files Written

| Path | Purpose |
|------|---------|
| `%USERPROFILE%\.agent-presence\config.json` | Only if you explicitly configure the app |

### Processes Checked

The app runs a PowerShell command every 2 seconds to list running processes:

```powershell
Get-CimInstance Win32_Process | Select-Object Name,CommandLine | ConvertTo-Json -Compress
```

This reads the process list to detect which agents are running. It does NOT read process memory, inject code, or modify any processes.

### Network Access

- **None at runtime.** The app only communicates with Discord via local IPC pipes (`\\.\pipe\discord-ipc-*`).
- No HTTP requests, no telemetry, no external connections.

## What It Does NOT Do

- Does NOT read your code or source files
- Does NOT read your terminal history or input
- Does NOT send data to any external server
- Does NOT modify any agent files or sessions
- Does NOT install startup entries or services
- Does NOT require administrator permissions
- Does NOT inject into other processes
- Does NOT capture screenshots or screen content

## How Detection Works

1. **Session files**: The app reads metadata from agent session files (JSONL for Codex/Pi, SQLite for OpenCode). These files contain project name, model, token usage, and activity status.

2. **Process detection**: The app checks running processes to confirm agents are still active. It matches process names and command lines against known patterns (e.g., `opencode.exe`, `codex.js`, `pi-node`).

3. **Agent switching**: When you switch between terminals, the app detects which agent's session file was most recently updated and switches Discord status accordingly.

## Discord Communication

The app communicates with Discord via local IPC pipes. It sends:
- Application ID (for which Discord app to show)
- Activity details (agent name, project, model, etc.)
- Timestamps

It does NOT send:
- File contents
- Source code
- Personal information
- Any data from your computer besides the activity status

## Open Source

The full source code is available at:
https://github.com/Zenolitee/AgentPresence

You can verify this report by reading the source code in `src/main.rs`.

## Building From Source

If you prefer to build from source instead of using the prebuilt binary:

```powershell
git clone https://github.com/Zenolitee/AgentPresence.git
cd AgentPresence
cargo build --release
```

The binary will be at `target\release\multi-agent-presence.exe`.

## Configuration

The app reads configuration from `%USERPROFILE%\.agent-presence\config.json`. All detection flags default to `true`. You can disable specific agent detection by setting the corresponding flag to `false`:

```json
{
  "detect_codex": true,
  "detect_pi": true,
  "detect_opencode": true,
  "detect_omp": true,
  "detect_processes": true
}
```

## License

MIT License - see [LICENSE](LICENSE) for details.
