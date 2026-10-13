# Fresh native cursor-correct retake; silent GIF source with the updated context-help/mode-label binary.
# Capture owner must pass README isolation checks and use a fresh output; final media review is pending.
Require herdr
Output "/tmp/opencode/xrd/navigation-search-cursor.mp4"
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
Type "j"
Sleep 1s
Type "k"
Sleep 1s
Type "l"
Sleep 1s
Type "h"
Sleep 1s
Tab
Sleep 1s
Shift+Tab
Sleep 1s
Type "/"
Sleep 400ms
Type "o"
Sleep 400ms
Type "d"
Sleep 400ms
Type "x"
Sleep 400ms
Sleep 2s
# Finish search without focusing or closing the popup.
Enter
Sleep 2s
Type "j"
Sleep 2s
# Confirm focus only on the second Enter.
Enter
Sleep 3s
Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
