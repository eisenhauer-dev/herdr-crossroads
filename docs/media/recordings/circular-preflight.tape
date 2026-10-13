# Owner-only native validation after new circular-navigation binary gates pass.
# Verify default keys, Editor focus, Auto, native order/counts and ten odx matches.
# Never confirm pane focus or save a theme; compare native snapshots/theme files.
Require herdr
Output "/tmp/opencode/xrd/circular-preflight.mp4"

Set Shell bash
Set FontFamily "DejaVu Sans Mono"
Set FontSize 16
Set Columns 160
Set Rows 40
Set Padding 12
Set Framerate 25
Set TypingSpeed 1ms
Set CursorBlink false

Env HOME "/tmp/opencode/xrd/home"
Env HERDR_CONFIG_PATH "/tmp/opencode/xrd/home/.config/herdr/config.toml"

Hide
Type "herdr --session film"
Enter
Sleep 3s
Type "herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30; clear"
Enter
Sleep 3s
Show
Sleep 2s
# Full pane list: first Editor to last Health Checks, then first Editor.
Type "k"
Sleep 1.2s
Type "j"
Sleep 1.2s
# Tab blocks: first Frontend to last Codex Deploy / deploy.sh, then first.
Type "h"
Sleep 1.2s
Type "l"
Sleep 1.2s
# Workspace blocks: Atlas Shop to Harbor Ops / deploy.sh, then Atlas Shop.
Shift+Tab
Sleep 1.2s
Tab
Sleep 1.2s
# Select Help: first next-pane entry to last close entry, then first; return.
Type "?"
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "q"
Sleep 300ms
# Theme list: Auto to last offered theme, then Auto; no save.
Type "t"
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Space
Sleep 1s
# The table preview uses the same theme cursor: Auto to last offered theme and back.
Type "k"
Sleep 1s
Type "j"
Sleep 1s
# Themes Help wraps without losing its calling Auto table-preview return state.
Type "?"
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "q"
Sleep 1s
Space
Sleep 1s
Escape
Sleep 300ms
# Filtered pane list: Enter ends editing only, not focus confirmation.
Type "/"
Sleep 300ms
Set TypingSpeed 50ms
Type "odx"
Set TypingSpeed 1ms
Sleep 2s
Enter
Sleep 1s
# First visible Codex Docs wraps to last visible Health Checks and back.
Type "k"
Sleep 1.2s
Type "j"
Sleep 1.2s
# Filtered tab blocks: Web App to Codex Deploy / deploy.sh, then Web App / Codex Docs.
Type "h"
Sleep 1.2s
Type "l"
Sleep 1.2s
# Filtered workspaces: Harbor Web to Harbor Ops / deploy.sh, then Harbor Web / Codex Docs.
Shift+Tab
Sleep 1.2s
Tab
Sleep 1.2s
# Clearing the filter retains Codex Docs selection; host focus is still Editor.
Type "r"
Sleep 1s
Type "q"
Sleep 2s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
