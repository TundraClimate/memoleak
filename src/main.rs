use std::fmt::Display;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
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

struct Stash {
    stash: Vec<Memo>,
}

impl Stash {
    fn new() -> Self {
        Self { stash: vec![] }
    }

    fn push(&mut self, memo: Memo) {
        self.stash.push(memo);
    }

    fn remove(&mut self, idx: usize) {
        self.stash.remove(idx);
    }
}

struct Memo {
    original_path: PathBuf,
    content_buffer: String,
    content_hash: u64,
}

impl Memo {
    fn new<P: AsRef<Path>>(original_path: P) -> Self {
        Self {
            original_path: original_path.as_ref().to_path_buf(),
            content_buffer: String::new(),
            // String::new hash
            content_hash: 3476900567878811119,
        }
    }

    fn with_content<P: AsRef<Path>>(original_path: P) -> Result<Self, Error> {
        let mut memo = Memo::new(original_path);

        memo.refresh()?;

        Ok(memo)
    }

    fn read_latest_content(&self) -> Result<String, Error> {
        fs::read_to_string(&self.original_path).map_err(|e| {
            Error::new(format!(
                "A file reading failed: '{}'({})",
                self.original_path.to_string_lossy(),
                e.kind()
            ))
        })
    }

    fn create_latest_hash(&self) -> Result<u64, Error> {
        let mut hasher = DefaultHasher::new();

        self.read_latest_content()?.hash(&mut hasher);

        Ok(hasher.finish())
    }

    fn eq_origin(&self) -> bool {
        Some(self.content_hash) == self.create_latest_hash().ok()
    }

    fn refresh(&mut self) -> Result<(), Error> {
        if !self.eq_origin() {
            self.content_buffer = self.read_latest_content()?;
            self.content_hash = self.create_latest_hash()?;
        }

        Ok(())
    }
}
