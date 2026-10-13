# Silent native capture; owner restores isolated Auto before this take.
# Owner verifies Auto at index 0 and Dracula at index 6; this take saves Dracula.
# Help, preview and theme save must not change the host's focused pane.
Require herdr
Output "/tmp/opencode/xrd/project-help-themes.mp4"

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
Sleep 1s
Type "?"
Sleep 3s
Type "/"
Set TypingSpeed 50ms
Type "focus"
Set TypingSpeed 1ms
Sleep 3s
Enter
Sleep 2s
Escape
Sleep 300ms
Type "t"
Sleep 3s
Type "?"
Sleep 3s
Escape
Sleep 1s
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Space
Sleep 3s
# Nested Help returns to the pending table preview, not to the theme list.
Type "?"
Sleep 2s
Type "q"
Sleep 1s
Space
Sleep 1s
# Save Dracula and return to Select without focusing a pane.
Enter
Sleep 7s

Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
