# Silent native capture; owner must pass new circular-navigation binary gates first.
# Default keys, Editor focused, isolated Auto selected; no focus or theme save.
# Actual decoded transitions, not nominal holds, determine later narration edits.
Require herdr
Output "/tmp/opencode/xrd/circular-navigation.mp4"

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
# Back at Atlas Shop / Frontend / Editor, with empty search.
Sleep 4s
# Pane boundary: first Editor to last Health Checks, then back to Editor.
Type "k"
Sleep 1.2s
Type "j"
Sleep 1.2s
# Tab boundary: first Frontend to last Codex Deploy / deploy.sh, then back.
Type "h"
Sleep 1.2s
Type "l"
Sleep 1.2s
# Workspace boundary: first Atlas Shop to last Harbor Ops / deploy.sh, then back.
Shift+Tab
Sleep 1.2s
Tab
Sleep 1.2s
# Theme boundary: Auto to the last offered theme, then back to Auto; cancel only.
Type "t"
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Escape
Sleep 300ms
# Select Help boundary: first next-pane entry to last close entry, then back.
Type "?"
Sleep 1s
Type "k"
Sleep 1s
Type "j"
Sleep 1s
Type "q"
Sleep 300ms
# Editor remains selected and focused; Auto remains unsaved and unchanged.
Sleep 2s

Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
