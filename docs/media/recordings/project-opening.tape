# Silent native capture; owner prepares Atlas Shop / Frontend / Editor first.
# Keep launch visible: the popup must not already be open when the take begins.
Require herdr
Output "/tmp/opencode/xrd/project-opening.mp4"

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
Sleep 2s
Set TypingSpeed 20ms
Type "herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30; clear"
Enter
Set TypingSpeed 1ms
Sleep 4s

Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
