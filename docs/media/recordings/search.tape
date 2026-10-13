# Provisional silent, caption-free native take; owner preflight required (README.md).
Require herdr
Output "/tmp/opencode/xrd/search.mp4"

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
Type "/"
Sleep 1.5s
Type "o"
Sleep 1.5s
Type "d"
Sleep 1.5s
Type "x"
Sleep 1.5s
# Hold pane, workspace, and tab matches in their original order.
Sleep 9s
# Finish editing without focusing or closing the popup.
Enter
Sleep 3s
Type "j"
# Hold the selected Codex Frontend row.
Sleep 4s
# Confirm focus and hold the actual Harbor Development destination.
Enter
Sleep 8s

# Exit only this attached client; leave the server and topology running.
Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
