use super::*;

const CWD: &str = "/ws";

fn owned() -> Vec<PathBuf> {
    vec![PathBuf::from("/ws/src/mine"), PathBuf::from("/ws/notes.md")]
}

fn refused(command: &str) -> String {
    refusal(command, Path::new(CWD), &owned())
        .unwrap_or_else(|| panic!("`{command}` should be refused"))
}

fn permitted(command: &str) {
    let verdict = refusal(command, Path::new(CWD), &owned());
    assert!(verdict.is_none(), "`{command}` should run: {verdict:?}");
}

mod commands;

mod shell;

mod directories;
