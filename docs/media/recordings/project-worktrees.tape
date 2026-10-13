# Silent native capture; owner retains the existing native linked relationship.
# Start at Atlas Shop / Frontend / Editor with default keys.
Require herdr
Output "/tmp/opencode/xrd/project-worktrees.mp4"

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
# Hold the native linked marker and feature/NO-TICKET-atlas-review branch.
Sleep 5s
Tab
# Select linked Atlas Checkout / Checkout without changing host focus.
Sleep 7s
Shift+Tab
# Return selection to Atlas Shop / Frontend / Editor without focus confirmation.
Sleep 4s
Type "q"
Sleep 2s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
