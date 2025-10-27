use std::fmt::Display;
use std::path::PathBuf;
use std::process;
use std::sync::LazyLock;

fn main() {
    if let Err(e) = setup() {
        fatal_err(format!("Local setup failed: {e}"));
    }
}

fn fatal_err<S: AsRef<str>>(s: S) -> ! {
    eprintln!("[ERR] {}", s.as_ref());

    process::exit(1)
}

struct Error(String);

impl Error {
    fn new<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref().to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

const APP_DATA_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_local_dir()
        .unwrap_or_else(|| fatal_err("A data dir is not found"))
        .join("memoleak")
});

fn setup() -> Result<(), Error> {
    Ok(())
}
