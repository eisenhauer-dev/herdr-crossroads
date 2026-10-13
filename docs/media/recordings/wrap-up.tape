# Provisional raw silent, caption-free native closing take; owner preflight required (README.md).
Require herdr
Output "/tmp/opencode/xrd/wrap-up.mp4"

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
# Close only Crossroads; hold the clean, unchanged Welcome host.
Type "q"
Sleep 3s

# Exit only this attached client; the editorial docs card is a separate edit.
Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
