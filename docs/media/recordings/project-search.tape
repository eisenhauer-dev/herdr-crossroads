# Silent native capture; default keys and owner-verified ten odx matches.
# Preserve native match order; narration alignment belongs to final composition.
Require herdr
Output "/tmp/opencode/xrd/project-search.mp4"

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
# Hold Search mode before the first Enter, including extra explanation time.
Sleep 27s
# First Enter only ends editing; it must leave Crossroads open.
Enter
Sleep 3s
Type "j"
# Select the second native match: Harbor Web / Web App / Codex UI.
Sleep 2s
# Only this second Enter confirms focus and closes the popup.
Enter
Sleep 3s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
