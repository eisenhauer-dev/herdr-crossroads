# Silent native capture; owner must pass new circular-navigation binary gates first.
# After Dracula save, restore Atlas Shop / Frontend / Editor without a theme reset.
Require herdr
Output "/tmp/opencode/xrd/circular-closing.mp4"

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
# Close only Crossroads; the actual Editor host remains focused.
Type "q"
Sleep 3s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
