# Provisional silent, caption-free context-help recapture; owner preflight required (README.md).
# Recapture with the updated binary for a consistent mode badge across all scenes.
# Do not execute until the owner verifies context-aware help, passes tests/gates, and permits capture.
Require herdr
Output "/tmp/opencode/xrd/navigation-context.mp4"

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
Sleep 3s
# Selection-only pane and tab movement; no pane confirmation.
Type "j"
Sleep 1.5s
Type "k"
Sleep 1.5s
Type "l"
Sleep 1.5s
Type "h"
Sleep 1.5s
Down
Sleep 1.5s
Up
Sleep 1.5s
Right
Sleep 1.5s
Left
Sleep 1.5s
Tab
Sleep 2s
Shift+Tab
Sleep 2s
# Return selection to Welcome; host focus remains unchanged.
Sleep 4s

# Close only the popup, then exit only this attached client.
Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
