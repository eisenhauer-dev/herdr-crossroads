# Status colors

Crossroads uses static Catppuccin Mocha colors by default. These values may not match a different Herdr theme; Crossroads does not synchronize colors with Herdr's current theme.

| Status | Default |
| --- | --- |
| `working` | `#f9e2af` |
| `blocked` | `#f38ba8` |
| `done` | `#94e2d5` |
| `idle` | `#a6e3a1` |
| `unknown` (including unrecognized status text) | `#6c7086` |

Put an optional `status-colors.json` in the plugin config directory that Herdr injects as `HERDR_PLUGIN_CONFIG_DIR`. To print the directory path, run `herdr plugin config-dir herdr-crossroads` (this Herdr command may create the directory, but Crossroads never creates the file). For example:

```json
{
  "working": "#123ABC",
  "blocked": "#f38ba8"
}
```

Only the five keys in the table are accepted. Values must be strings in `#RRGGBB` form (hexadecimal digits are case-insensitive). Missing keys retain their defaults. With no config directory or no file, all defaults apply; Crossroads never creates or modifies the file. The file is loaded once per popup opening, not on subsequent redraws. Invalid JSON, an unknown key, an invalid color, or a file read error stops startup with an error naming `status-colors.json` rather than silently falling back. These configurable colors apply only to the Status column text in wide layouts and to the status text following ` | ` in Agent/Pane Name in compact layouts. Search matches in workspace, tab, and agent/pane names use a separate fixed accent color (`#b4befe`) and underline. Rows without an agent have no inline status.
