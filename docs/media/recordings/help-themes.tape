# Provisional silent, caption-free planned context-help take; owner preflight required (README.md).
# HELD: recording is forbidden until context-aware help is verified and the owner permits execution.
# Owner must pass updated-binary tests/gates and recapture all scenes for a consistent mode badge.
# Precondition: isolated theme initially Auto (index 0); verify the current live palette
# places visible Dracula at index 6 before executing. Do not assume future palette indexes.
# Nominal visible holds total 32.7s plus typing; owner must verify actual exported timing.
Require herdr
Output "/tmp/opencode/xrd/help-themes.mp4"

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
# Finish Help search editing; this Enter must not focus a pane.
Enter
Sleep 2s
# Close listed Help only after editing has finished.
Escape
Sleep 300ms
Type "t"
Sleep 3s
# Planned Themes-context Help from the theme list.
Type "?"
Sleep 3s
Escape
Sleep 1s
# Six separate steps from preflight-verified Auto to visible Dracula.
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
# Planned Themes-context Help while retaining the table-preview return state.
Type "?"
Sleep 2s
Type "q"
Sleep 1s
Space
Sleep 1s
# Save Dracula and return to Select; this Enter must not focus a pane.
Enter
Sleep 7s

# Close only the popup, then exit only this attached client.
Hide
Type "q"
Sleep 300ms
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
