# Silent raw continuous take, not final edit timing. Owner starts Auto,
# Atlas Shop / Frontend / Editor; discovers IDs from the owned film snapshot.
# Compose title/cards/hierarchy over intro; popup must open only after its speech.
# Measure visible popup readiness, then start navigation speech 1–2s later.
Require herdr
Output "/tmp/opencode/xrd/guided-project-flow.mp4"

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
# Fully pretyped on the actual host, never typed visibly or entered while hidden.
Type "herdr --session film plugin pane open --plugin herdr-crossroads --entrypoint crossroads --width 142 --height 30; clear"
Show
Sleep 2s
Enter
Sleep 13s
# Pane selection only; host remains Editor.
Down
Sleep 6s
# Frontend -> Tests / Unit Tests, still no host focus change.
Right
Sleep 5s
# One workspace step: Atlas Checkout / Checkout / Checkout Editor.
# End navigation with editorial mapping; worktrees continues this same view.
Tab
Sleep 18s
Type "/"
Sleep 300ms
Set TypingSpeed 180ms
Type "cou"
Set TypingSpeed 1ms
Sleep 9s
# End editing only; retain selected Checkout Editor among five fuzzy results.
Enter
Sleep 2s
# Only this second search Enter focuses Checkout Editor and closes the popup.
Enter
Sleep 5s

Hide
# Detach the attached client only, leaving the owned film server for source two.
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
