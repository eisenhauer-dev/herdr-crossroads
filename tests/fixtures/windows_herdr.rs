//! Native fake CLI used only by the isolated Windows tests (never a real Herdr).
use std::io::Write;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(path) = std::env::var_os("CROSSROADS_TEST_LOG") {
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(log, "{}", args.join(" ")).unwrap();
    }
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["api", "snapshot"] => {
            if std::env::var_os("CROSSROADS_TEST_STALL").is_some() {
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            println!("{}", include_str!("topology.json"));
        }
        ["pane", "list"] => println!("{{\"result\":{{\"type\":\"pane_list\",\"panes\":[{{\"pane_id\":\"p-b\"}},{{\"pane_id\":\"p-other\"}}]}}}}"),
        ["worktree", "list", ..] => println!("{{\"result\":{{\"type\":\"worktree_list\",\"worktrees\":[]}}}}"),
        _ => std::process::exit(2),
    }
}
