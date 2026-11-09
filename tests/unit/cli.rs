#[test]
fn setup_interpreter_and_argv_are_native_without_a_shell() {
    let command = crate::cli::setup_command();
    assert_eq!(
        command.get_program(),
        if cfg!(windows) { "python" } else { "python3" }
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["scripts/setup-keybind.py"]
    );
    assert_eq!(
        command.get_envs().count(),
        0,
        "inherit the action environment"
    );
    assert!(
        command.get_current_dir().is_none(),
        "inherit the plugin root"
    );
}
