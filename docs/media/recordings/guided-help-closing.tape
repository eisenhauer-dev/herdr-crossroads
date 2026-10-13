# Silent raw take; retain actual Checkout Editor host focus from source one.
# Start saved Auto, no context reset. Owner measures native transitions and
# records theme file after FIRST save (Latte), then SECOND save (Auto/removal).
Require herdr
Output "/tmp/opencode/xrd/guided-help-closing.mp4"

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
# Reconstruct filtered Select with native input, not a host focus action.
Type "/"
Type "cou"
Enter
Sleep 500ms
Show
Sleep 2s
Type "?"
Sleep 3.5s
Type "q"
Sleep 2s
Type "t"
Sleep 2s
# Auto -> catppuccin (same palette) -> Latte (one visible palette change).
Type "jj"
Sleep 2s
Space
Sleep 4s
# Save directly FROM Themes table preview, never return to the list first.
Enter
Sleep 2.5s
# A real second opening starts at saved Latte, not at Auto.
Type "t"
Sleep 2s
Type "kk"
Sleep 2s
# Save restored Auto/Herdr default from Themes, without pane focus.
Enter
Sleep 2s
Type "q"
Sleep 3s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
