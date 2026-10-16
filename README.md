# Herdr Crossroads

A keyboard-driven pane navigator for [Herdr](https://herdr.dev): search panes,
tabs, workspaces, or agent names and focus the pane you need without reorganizing
your session.

## Contents

- [Demo](#crossroads-demo---voiceover-by-elevenlabsio)
- [Features](#features)
- [Requirements and compatibility](#requirements-and-compatibility)
- [Installation](#installation)
- [Controls](#controls)
- [Configuration](#configuration)
- [Update and uninstall](#update-and-uninstall)
- [Documentation and license](#documentation-and-license)

## Crossroads Demo - voiceover by elevenlabs.io

https://github.com/user-attachments/assets/c86b3c7c-718b-4033-82f8-8910d97d7278

The demo uses AI-generated **ElevenLabs Free narration for noncommercial use only**.
The software's MIT license does not grant commercial reuse rights to that narration;
see [media terms, attribution and provenance](docs/media/README.md#narration-use-and-attribution).

## Features

- Workspace/tab/pane navigation with stable pane-ID focus, including duplicate names.
- Live, case-insensitive subsequence search without requerying or reordering Herdr topology.
- [Linked worktrees](docs/worktrees.md) grouped beside their source workspace with `↳` and branch context.
- Live local agent statuses, [theme previews](docs/themes.md), customizable keys, and searchable help.
- **Navigation-only:** no creation, movement, renaming, or deletion of session topology.
  Only confirmation changes focus; explicitly saving a theme changes plugin-owned files.
- Opening/searching uses local CLI/socket access and files, not network requests or updates.
  Crossroads does not change Herdr configuration, native prefix `g`/`t` bindings, or other plugins.

<a id="navigation-and-search"></a>

Historical native previews (not current release verification):
[navigation and themes provenance](docs/media/recordings/native-loops.md).

![Native navigation and live search](docs/media/native-navigation.gif)
![Native linked-worktree selection](docs/media/linked-worktrees.gif)
![Native theme preview and save](docs/media/native-themes.gif)

## Requirements and compatibility

- **Linux x86_64 with glibc** (`x86_64-unknown-linux-gnu` only), `stty` on `PATH`,
  and a running Herdr session; manifest minimum **Herdr 0.9.1**.
- **80×15 popup content cells** minimum, excluding host borders; enlarge your terminal if needed.
- Managed installation: Git and **Python 3.11+** (standard library only); no Rust/Cargo.
  Source builds: Git, Rust/Cargo, and a system linker.
- No macOS, Windows, other-architecture, or SSH support is claimed; no minimum glibc
  or distribution compatibility guarantee. See [runtime limits](docs/builds.md#runtime-and-verification-limits)
  and [terminal limits](docs/keymaps.md#terminal-limits).
- **[0.1.0](https://github.com/eisenhauer-dev/herdr-crossroads/releases/tag/v0.1.0)
  is the first regular Linux release**, published as Latest with reviewed clean CI assets.
  [Public installation evidence](docs/installation.md#regular-010-public-installation)
  distinguishes pinned native navigation from unpinned CLI installation; no `0.1.0` upgrade cycle is claimed.

## Installation

Install [Herdr](https://herdr.dev) first. The primary Crossroads installation command is:

```sh
herdr plugin install eisenhauer-dev/herdr-crossroads
```

- This follows the public default branch; the checkout's version-pinned hook downloads
  its matching Linux archive/checksum. The unpinned command passed isolated CLI
  installation on Herdr 0.9.5; native navigation was tested separately with a pinned release.
- Review the source, manifest, and assets before approving hook execution:
  installation executes code and registers a plugin globally for your user, not in a sandbox.
- See [binary installation](docs/installation.md) for trusted local assets, pinned test
  prereleases, validation, and version-specific Herdr behavior. There is no short-name registry.

Alternatively, explicitly clone, build, and link a trusted source checkout:

```sh
git clone https://github.com/eisenhauer-dev/herdr-crossroads.git
cd herdr-crossroads
cargo build --release --locked
herdr plugin link "$PWD"
```

- Linking does not run build hooks. Keep the checkout and `target/release/herdr-crossroads`
  in place; Cargo may download dependencies on a fresh machine.
- From a shell in your running Herdr session, open Crossroads:

```sh
herdr plugin pane open --plugin herdr-crossroads --entrypoint crossroads
```

- The default popup is half-size. For a trusted local checkout, `width = "90%"` and
  `height = "90%"` belong in the manifest's `[[panes]]` table before registration,
  not CLI flags; neither size guarantees 80×15 content cells.
- No launch binding is installed. For an optional host shortcut, follow
  [Herdr's versioned configuration guide][herdr-config], check `prefix+?` for conflicts,
  and preserve existing navigation/close bindings. `keymaps.jsonc` changes only the plugin UI.
- These source clone/build/link and personal-host configuration examples remain unexecuted;
  isolated installation evidence is recorded in the [installation guide](docs/installation.md).

## Controls

<a id="select-defaults"></a>

| Action | Default keys |
| --- | --- |
| Next / previous pane | `j` / `k`, or Down / Up |
| Next / previous tab | `l` / `h`, or Right / Left |
| Next / previous workspace | Tab / Shift+Tab |
| Search / reset filter | `/` / `r` |
| Keys help / themes | `?` / `t` |
| Focus selected pane and close | Enter |
| Close without focus | Esc or `q` |

- Movement wraps through visible panes/blocks without changing host focus.
- While editing search, printable keys are text; Enter/Esc ends editing but retains
  the filter. Press Enter again to focus; no matches means no focus.
- Themes: `j`/`k` or arrows to preview, Space for list/table view, Enter to save,
  Esc/`q` to cancel. `?` shows the current view's effective keys.
- See [complete controls, customization and input limits](docs/keymaps.md#actions-and-defaults).

## Configuration

<a id="configuration-and-complete-defaults-overview"></a>

No configuration is required. Locate the plugin-owned directory with
`herdr plugin config-dir herdr-crossroads` (the host may create it), then reopen
Crossroads after editing.

| Optional file | Reference |
| --- | --- |
| `keymaps.jsonc` | [Action bindings, schema and defaults](docs/keymaps.md) |
| `theme.jsonc` | [Themes, Auto fallback and persistence](docs/themes.md) |
| `status-colors.jsonc` | [Status colors and precedence](docs/status-colors.md) |

- JSONC supports comments and trailing commas; legacy `.json` is read only when
  `.jsonc` is absent, never when it is invalid or unreadable.
- Keymaps and status colors are read-only. Saving a theme writes only plugin-owned
  theme files; explicitly saving Auto removes both theme files. See the references for failure behavior.

## Update and uninstall

- Close Crossroads before updating, rebuilding, or uninstalling; uninstall does not stop an open popup.
- Managed installs: Herdr 0.9.5 provides `herdr plugin update herdr-crossroads`;
  0.9.3 instead needs reinstalling a reviewed ref with matching assets.
  See [managed lifecycle and evidence](docs/installation.md#native-distribution-acceptance).
- Linked checkout: inspect local changes and the intended revision, update it,
  then rerun `cargo build --release --locked`. The link still points to that checkout.
- Uninstall: `herdr plugin uninstall herdr-crossroads`. A local link leaves the
  checkout intact; managed checkout retention depends on Herdr's version and live leases.
  Plugin config/state remains: back up your files before any manual cleanup.
- Crossroads has no automatic updater; updating Herdr itself is a separate operation.

## Documentation and license

- [Troubleshooting by symptom](docs/troubleshooting.md)
- [Architecture](docs/architecture.md), [contributing and testing](CONTRIBUTING.md)
- [Linux artifacts and verification limits](docs/builds.md), [release preparation](docs/releases.md)
- Software: [MIT](LICENSE), copyright 2026 SimonEisenhauer. Binary distributions
  retain `LICENSE` and `THIRD-PARTY-NOTICES.html`; demo narration has
  [separate provider restrictions](docs/media/README.md#narration-use-and-attribution).

[herdr-config]: https://github.com/herdrdev/herdr/blob/v0.9.3/docs/versions/0.9.1/website/src/content/docs/configuration.mdx
