# Status colors

Crossroads uses the selected plugin theme's status colors by default. With Auto (Herdr), it reads a simple saved Herdr theme choice when unambiguous; otherwise it falls back to Catppuccin Mocha. The table shows the Mocha fallback values.

| Status | Default |
| --- | --- |
| `working` | `#f9e2af` |
| `blocked` | `#f38ba8` |
| `done` | `#94e2d5` |
| `idle` | `#a6e3a1` |
| `unknown` (including unrecognized status text) | `#6c7086` |

Put an optional `status-colors.jsonc` in the plugin config directory that Herdr injects as `HERDR_PLUGIN_CONFIG_DIR`. Crossroads reads legacy `status-colors.json` only if `.jsonc` is absent; a present invalid or unreadable `.jsonc` never falls back to `.json`. To print the directory path, run `herdr plugin config-dir herdr-crossroads` (this Herdr command may create the directory, but Crossroads never creates the file). For example:

```jsonc
{
  // Other statuses continue to follow the active theme.
  "working": "#123ABC",
  "blocked": "#f38ba8",
}
```

Only the five keys in the table are accepted. Values must be strings in `#RRGGBB` form (hexadecimal digits are case-insensitive). Both file extensions accept JSON plus `//` and `/* … */` comments and trailing commas in objects/arrays, but no other JSON5 extensions. Duplicate field names are rejected. Explicit keys override the selected theme's corresponding status color; missing keys follow the theme. With no config directory or no file, theme colors apply; Crossroads never creates, modifies, migrates, or deletes either status-color file. The file is loaded once per popup opening, not on subsequent redraws.

Invalid JSONC, unknown keys, invalid colors, duplicate fields, or read errors open the [common configuration error page](keymaps.md#conflicts-and-error-page) rather than aborting or silently using defaults. It collects all independently detectable startup errors across keymaps, theme, and status colors. All recoverable status fields, including duplicate occurrences, are checked; unreadable or unparseable files contribute only IO or syntax diagnostics. Each error kind has its own bold, error-colored `ERROR:` heading, then one source row, then a simple diagnostic list. The source row shows the filename in normal text, two spaces, and the full selected file path in the active theme's dim color. The path is `.jsonc` when present, otherwise legacy `.json`; it is never repeated in diagnostic lines. Source rows never wrap: long paths clip on the right with `…` using Unicode cell widths, preserving the filename where possible. Extremely narrow rows omit the path or clip the filename safely. Plain-text output has the same single clipped row without styling. The source row scrolls with its block. Stacked blocks are separated by horizontal lines. The page uses fixed ↑/k, ↓/j and Enter/Esc/q controls without configured action resolution or pane focus. At three or more rows, the navigation footer and full-width separator stay at the popup bottom while content scrolls independently; one or two rows prioritize diagnostics with ERROR markers and omit headings/source rows. Interactive content below 80×15 adds a first `ERROR: Popup too small` block with required/actual dimensions and enlarge/reopen advice, alongside all configuration errors, but no configuration path. Resize updates or removes that block; noninteractive output does not add it.

These configurable colors apply only to the Status column text in wide layouts and to the status text following ` | ` in Agent/Pane Name in compact layouts. Search matches in workspace, tab, and agent/pane names are underlined in the theme accent (Mocha keeps its previous `#b4befe` accent). Mocha's ordinary text and panel use `#cdd6f4` and `#181825` respectively; `terminal` alone inherits the terminal's current default text and background colors. Rows without an agent have no inline status.
