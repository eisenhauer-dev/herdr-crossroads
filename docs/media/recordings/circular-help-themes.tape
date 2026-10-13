# Silent native capture; owner must pass new circular-navigation binary gates first.
# Owner verifies isolated Auto at index 0 and Dracula at index 6 before capture.
# Help/list/preview boundary moves preserve Editor; only final Themes Enter saves.
Require herdr
Output "/tmp/opencode/xrd/circular-help-themes.mp4"

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
# Theme-list boundary returns the pending cursor to Auto before Dracula movement.
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "?"
Sleep 3s
# Themes Help boundary: first next-theme entry to last show-keys entry and back.
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "q"
Sleep 1s
# Six forward steps from owner-verified Auto to Dracula; no wrapping here.
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
# Nested Themes Help must return to this exact pending Dracula table preview.
Type "?"
Sleep 2s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
# Independently filter Themes Help to multiple theme actions; wrap visible entries.
Type "/"
Sleep 300ms
Set TypingSpeed 50ms
Type "theme"
Set TypingSpeed 1ms
Sleep 1s
# This Enter ends Help editing only; it cannot save or focus.
Enter
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "r"
Sleep 300ms
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
