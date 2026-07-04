# Configuration Guide

MultiAgent-Presence reads its config from:

```text
%USERPROFILE%\.agent-presence\config.json
```

On Windows, that usually means:

```text
C:\Users\<you>\.agent-presence\config.json
```

Create the config from the repo default:

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.agent-presence"
Copy-Item .\config.example.json "$env:USERPROFILE\.agent-presence\config.json"
```

Open it:

```powershell
notepad "$env:USERPROFILE\.agent-presence\config.json"
```

## Default Config

```json
{
  "client_id": "1522704011491545159",
  "opencode_client_id": "1522861438778212463",
  "pi_client_id": "1522861633909821581",
  "large_image": "codex-logo",
  "large_text": "Codex",
  "claude_large_image": "claude-logo",
  "claude_large_text": "Claude Code",
  "opencode_large_image": "opencode-logo",
  "opencode_large_text": "OpenCode",
  "pi_large_image": "pi-logo",
  "pi_large_text": "Pi",
  "hide_project": false,
  "hide_model": false,
  "show_branch": true,
  "flavor_text": true,
  "show_activity": true,
  "show_surface": true,
  "show_plan": false,
  "show_tokens": true,
  "show_cost": true,
  "show_context": false,
  "show_limits": false,
  "priority_presence": true,
  "detect_processes": true,
  "detect_codex": true,
  "detect_pi": true,
  "poll_seconds": 2,
  "stale_seconds": 180,
  "codex_home": null,
  "pi_home": null
}
```

## Options

- `client_id`: Discord application ID used for app name and assets. The default is the shared AgentPresence app.
- `opencode_client_id`: Discord application ID used when OpenCode is detected.
- `pi_client_id`: Discord application ID used when Pi is detected.
- `large_image`: Discord Rich Presence asset key for Codex.
- `large_text`: hover text for the Codex image.
- `claude_large_image` / `claude_large_text`: Discord asset key and hover text used when Claude Code is detected.
- `opencode_large_image` / `opencode_large_text`: Discord asset key and hover text used when OpenCode is detected.
- `pi_large_image` / `pi_large_text`: Discord asset key and hover text used when Pi is detected.
- `hide_project`: hides the workspace folder name when `true`.
- `hide_model`: hides the model label when `true`.
- `show_branch`: shows the current git branch when available.
- `flavor_text`: rotates fun activity text when no specific activity is available.
- `show_activity`: shows coarse activity such as `Running command cargo test`, `Applying patch`, or `Searching the web`.
- `show_surface`: shows the detected Codex surface, such as `Codex CLI`.
- `show_plan`: shows the detected plan label, such as `Plus ($20/month)`.
- `show_tokens`: shows total token usage for the current session.
- `show_cost`: attempts to show estimated cost only when reliable pricing is available.
- `show_context`: shows latest context-window usage percentage.
- `show_limits`: shows 5-hour and 7-day quota usage percentages.
- `priority_presence`: republishes every 2 seconds so Codex activity stays visible above other Discord activities.
- `detect_processes`: enables fallback process command-line detection for Claude Code, OpenCode, and Pi.
- `detect_codex`: enables Codex JSONL session detection. Set to `false` when testing OpenCode while Codex is still running.
- `detect_pi`: enables Pi JSONL session detection.
- `poll_seconds`: refresh interval. Use `2` with `priority_presence`.
- `stale_seconds`: seconds before an inactive Codex session is treated as idle.
- `codex_home`: optional override for Codex home. Leave `null` for `%USERPROFILE%\.codex`.
- `pi_home`: optional override for Pi home. Leave `null` for `%USERPROFILE%\.pi`.

## Privacy Presets

Minimal:

```json
{
  "hide_project": true,
  "hide_model": true,
  "show_branch": false,
  "show_plan": false,
  "show_tokens": false,
  "show_context": false,
  "show_limits": false
}
```

## Agent Images

AgentPresence uses one Discord application and changes the Rich Presence image by setting the `assets.large_image` key for the detected agent. Upload matching Rich Presence assets to your Discord application:

```text
codex-logo
claude-logo
opencode-logo
pi-logo
```

The files in `assets/` are only source images for upload. Discord does not load those local files directly at runtime.

Structured Codex and Pi session detection is preferred when local session files are active. If no active local session is found and `detect_processes` is `true`, AgentPresence scans local process command lines for OpenCode, Claude Code, and Pi and switches the image accordingly.

Balanced:

```json
{
  "hide_project": false,
  "hide_model": false,
  "show_branch": true,
  "show_activity": true,
  "show_surface": true,
  "show_tokens": true,
  "show_plan": false,
  "show_context": false,
  "show_limits": false
}
```

Verbose:

```json
{
  "hide_project": false,
  "hide_model": false,
  "show_branch": true,
  "show_activity": true,
  "show_surface": true,
  "show_plan": true,
  "show_tokens": true,
  "show_context": true,
  "show_limits": true
}
```
