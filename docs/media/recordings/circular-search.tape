# Silent native capture; owner must pass new circular-navigation binary gates first.
# Owner verifies ten odx matches in native order; boundary checks are separate.
Require herdr
Output "/tmp/opencode/xrd/circular-search.mp4"

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
# Hold Search for the explanation; composition preserves the late-focus contract.
Sleep 27s
# First Enter only ends editing; the filter and popup remain.
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
