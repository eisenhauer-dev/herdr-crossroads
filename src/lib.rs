mod actions;
mod config;
mod focus;
mod input;
mod keymap;
mod presentation;
mod runtime;
mod search;
mod selection;
mod status_colors;
mod terminal;
mod theme;
mod topology;
mod transport;
#[cfg(windows)]
mod windows_pipe;

pub fn run() -> std::io::Result<()> {
    runtime::run()
}

#[cfg(test)]
#[path = "../tests/unit/actions.rs"]
mod test_actions;
#[cfg(test)]
#[path = "../tests/unit/input.rs"]
mod test_input;
#[cfg(test)]
#[path = "../tests/unit/keymap.rs"]
mod test_keymap;
#[cfg(test)]
#[path = "../tests/unit/mouse.rs"]
mod test_mouse;
#[cfg(test)]
#[path = "../tests/unit/presentation.rs"]
mod test_presentation;
#[cfg(test)]
#[path = "../tests/unit/runtime.rs"]
mod test_runtime;
#[cfg(test)]
#[path = "../tests/unit/search.rs"]
mod test_search;
#[cfg(test)]
#[path = "../tests/unit/selection.rs"]
mod test_selection;
#[cfg(test)]
#[path = "../tests/unit/status_colors.rs"]
mod test_status_colors;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
#[cfg(test)]
#[path = "../tests/unit/theme.rs"]
mod test_theme;
#[cfg(test)]
#[path = "../tests/unit/topology.rs"]
mod test_topology;
#[cfg(test)]
#[path = "../tests/unit/transport.rs"]
mod test_transport;
#[cfg(all(test, windows))]
#[path = "../tests/unit/windows.rs"]
mod test_windows;
