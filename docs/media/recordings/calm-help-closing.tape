# Silent continuous take; retain actual Checkout Editor host focus from flow.
# Owner must not restore Atlas Shop between takes. Start with saved Auto.
# Reopening resets UI selection; hidden native search reconstructs the context.
# Sleep values provide raw footage, not final narration or edit timestamps.
Require herdr
Output "/tmp/opencode/xrd/calm-help-closing.mp4"

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
Type "/"
Type "checkout"
Enter
Sleep 500ms
Show
Sleep 2s
Type "?"
Sleep 5s
# Close Help only; return to the same filtered selection and pane context.
Type "q"
Sleep 2s
Type "t"
Sleep 2.5s
# Auto -> catppuccin -> catppuccin-latte; only Latte changes the palette.
Type "jj"
Sleep 4s
Space
Sleep 5s
Space
Sleep 1.5s
# Save Latte and return to Select, without requesting pane focus.
Enter
Sleep 5s
# Close the popup visibly; actual host is still Checkout Editor.
Type "q"
Sleep 3s

Hide
Ctrl+b
Sleep 500ms
Type "q"
Sleep 1s
