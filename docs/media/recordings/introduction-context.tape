# Provisional silent, caption-free context-help recapture; owner preflight required (README.md).
# Recapture with the updated binary for a consistent mode badge across all scenes.
# Do not execute until the owner verifies context-aware help, passes tests/gates, and permits capture.
Require herdr
Output "/tmp/opencode/xrd/introduction-context.mp4"

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
Show
# Hold the actual neutral Atlas Overview Welcome host before opening.
Sleep 2s
Hide
Type "herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30; clear"
Enter
Sleep 3s
Show
Sleep 28s

# Close only the popup, then exit only this attached client.
Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
