# Configuration Guide

AgentPresence reads its config from:

```text
%USERPROFILE%\.codex-discord-presence\config.json
```

On Windows, that usually means:

```text
C:\Users\<you>\.codex-discord-presence\config.json
```

Create the config from the repo default:

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.codex-discord-presence"
Copy-Item .\config.example.json "$env:USERPROFILE\.codex-discord-presence\config.json"
```

Open it:

```powershell
notepad "$env:USERPROFILE\.codex-discord-presence\config.json"
```

## Default Config

```json
{
  "client_id": "1522704011491545159",
  "large_image": "codex-logo",
  "large_text": "Codex",
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
  "poll_seconds": 2,
  "stale_seconds": 180,
  "codex_home": null
}
```

## Options

- `client_id`: Discord application ID used for app name and assets. The default is the shared AgentPresence app.
- `large_image`: Discord Rich Presence asset key for the large image.
- `large_text`: hover text for the large image.
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
- `poll_seconds`: refresh interval. Use `2` with `priority_presence`.
- `stale_seconds`: seconds before an inactive Codex session is treated as idle.
- `codex_home`: optional override for Codex home. Leave `null` for `%USERPROFILE%\.codex`.

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

