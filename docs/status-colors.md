# Status colors

Crossroads uses the selected plugin theme's status colors by default. With Auto (Herdr), it reads a simple saved Herdr theme choice when unambiguous; otherwise it falls back to Catppuccin Mocha. The table shows the Mocha fallback values.

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

Only the five keys in the table are accepted. Values must be strings in `#RRGGBB` form (hexadecimal digits are case-insensitive). Explicit keys override the selected theme's corresponding status color; missing keys follow the theme. With no config directory or no file, theme colors apply; Crossroads never creates or modifies `status-colors.json`. The file is loaded once per popup opening, not on subsequent redraws. Invalid JSON, an unknown key, an invalid color, or a file read error stops startup with an error naming `status-colors.json` rather than silently falling back. These configurable colors apply only to the Status column text in wide layouts and to the status text following ` | ` in Agent/Pane Name in compact layouts. Search matches in workspace, tab, and agent/pane names are underlined in the theme accent (Mocha keeps its previous `#b4befe` accent). Mocha's ordinary text and panel use `#cdd6f4` and `#181825` respectively; `terminal` alone inherits the terminal's current default text and background colors. Rows without an agent have no inline status.
