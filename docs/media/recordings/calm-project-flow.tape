# Silent continuous native take; owner starts Atlas Shop / Frontend / Editor.
# Auto must resolve to catppuccin; use the accepted binary and isolated film env.
# Sleep values provide raw footage, not final narration or edit timestamps.
Require herdr
Output "/tmp/opencode/xrd/calm-project-flow.mp4"

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
Sleep 16s
# Selection only: Editor -> pnpm dev; host remains Editor.
Down
Sleep 6s
# One workspace step selects Atlas Checkout / Checkout / Checkout Editor.
Tab
Sleep 20s
Type "/"
Sleep 300ms
Set TypingSpeed 180ms
Type "checkout"
Set TypingSpeed 1ms
Sleep 10s
# First Enter ends query editing; no focus request or popup close.
Enter
Sleep 2s
# One wrap cycle within the four filtered Checkout rows.
Up
Sleep 1.5s
Down
Sleep 1.5s
# Only this Enter focuses Checkout Editor (w4:p1) and closes Crossroads.
Enter
Sleep 6s

Hide
# Popup is already closed: quit the attached client, not the server.
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
