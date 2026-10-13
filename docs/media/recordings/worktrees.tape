# Provisional silent, caption-free native take; owner preflight required (README.md).
# Prepared isolated film server, Atlas Overview Welcome focused, default keys.
Output "/tmp/opencode/xrd/worktrees.mp4"

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
Type "herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30"
Enter
Sleep 3s

Show
# Hold the native overview, linked-worktree marker, and branch label.
Sleep 5s
Tab
# Select Atlas Review without changing host focus.
Sleep 7s
Shift+Tab
# Return selection to Atlas Welcome, still without focusing it.
Sleep 4s
Type "q"
# Hold the actual unchanged host after closing Crossroads.
Sleep 2s

# Exit only this attached client; leave the server and topology running.
Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
