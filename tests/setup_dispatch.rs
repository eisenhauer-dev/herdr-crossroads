use std::process::{Command, Stdio};

#[test]
fn setup_dispatch_inherits_output_arguments_environment_and_exit_without_ui() {
    let root = if cfg!(windows) {
        std::env::temp_dir()
    } else {
        "/tmp/opencode".into()
    }
    .join(format!("crossroads setup dispatch {}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("target/release")).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_herdr-crossroads"),
        root.join(if cfg!(windows) {
            "target/release/herdr-crossroads.exe"
        } else {
            "target/release/herdr-crossroads"
        }),
    )
    .unwrap();
    std::fs::write(
        root.join("scripts/setup-keybind.py"),
        r#"import os, sys
assert sys.argv == ['scripts/setup-keybind.py']
assert os.environ['CROSSROADS_DISPATCH_TEST'] == 'inherited'
print('helper stdout', flush=True)
print('helper stderr', file=sys.stderr, flush=True)
sys.exit(int(os.environ['CROSSROADS_DISPATCH_EXIT']))
"#,
    )
    .unwrap();
    let manifest_command = include_str!("../herdr-plugin.toml")
        .lines()
        .find_map(|line| {
            line.strip_prefix("command = ")
                .filter(|line| line.contains("--setup-keybind"))
        })
        .unwrap();
    let argv: Vec<String> = serde_json::from_str(manifest_command).unwrap();
    // Herdr v0.9.5 plugin_command::program_for_cwd joins relative argv paths
    // to the absolute plugin root. Rust Command appends .exe on Windows.
    for code in [0, 23] {
        let output = Command::new(root.join(&argv[0]))
            .args(&argv[1..])
            .current_dir(&root)
            .env("CROSSROADS_DISPATCH_TEST", "inherited")
            .env("CROSSROADS_DISPATCH_EXIT", code.to_string())
            .env("HERDR_BIN_PATH", root.join("no-herdr"))
            .env("HERDR_SOCKET_PATH", root.join("no-socket"))
            .env("HERDR_CONFIG_PATH", root.join("no-config"))
            .env("HERDR_PLUGIN_CONFIG_DIR", root.join("no-plugin-config"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!root.join("no-plugin-config").exists());
        assert!(!root.join("no-config").exists());
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "helper stdout"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "helper stderr"
        );
    }
    for args in [
        ["--agent", "--setup-keybind"],
        ["--setup-keybind", "--agent"],
    ] {
        let output = Command::new(root.join(&argv[0]))
            .args(args)
            .current_dir(&root)
            .env("CROSSROADS_DISPATCH_TEST", "inherited")
            .env("CROSSROADS_DISPATCH_EXIT", "0")
            .env("HERDR_BIN_PATH", root.join("no-herdr"))
            .env("HERDR_SOCKET_PATH", root.join("no-socket"))
            .env("HERDR_CONFIG_PATH", root.join("no-config"))
            .env("HERDR_PLUGIN_CONFIG_DIR", root.join("no-plugin-config"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "setup/runtime ran: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--setup-keybind takes no arguments"),
            "{output:?}"
        );
        assert!(!root.join("no-plugin-config").exists());
        assert!(!root.join("no-config").exists());
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn setup_dispatch_refuses_extra_arguments_before_loading_runtime() {
    let missing = std::env::temp_dir().join(format!(
        "crossroads-missing-dispatch-{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-crossroads"))
        .args(["--setup-keybind", "unexpected"])
        .env("HERDR_BIN_PATH", "crossroads-no-such-herdr")
        .env("HERDR_PLUGIN_CONFIG_DIR", &missing)
        .env("HERDR_CONFIG_PATH", missing.join("config.toml"))
        .env("HOME", &missing)
        .env("XDG_CONFIG_HOME", &missing)
        .env_remove("HERDR_SOCKET_PATH")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--setup-keybind takes no arguments"));
}
