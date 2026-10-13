# Silent native capture; start on Atlas Shop / Frontend / Editor, default keys.
# Selection moves only; no pane focus confirmation in this take.
Require herdr
Output "/tmp/opencode/xrd/project-navigation.mp4"

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
# Selection returns to Atlas Shop / Frontend / Editor; host focus never changed.
Sleep 4s

Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
