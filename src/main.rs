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

    let mut stash = Stash::new();

    if let Err(e) = fill_stash_with_local(&mut stash) {
        fatal_err(format!("The memo stash refilling failed: {e}"));
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

static APP_DATA_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_local_dir()
        .unwrap_or_else(|| fatal_err("A data dir is not found"))
        .join("memoleak")
});

static MEMO_LIST_PATH: LazyLock<PathBuf> = LazyLock::new(|| APP_DATA_PATH.join("saved_files"));

fn setup() -> Result<(), Error> {
    if !APP_DATA_PATH.exists() {
        fs::create_dir_all(&*APP_DATA_PATH)
            .map_err(|e| Error::new(format!("APP_DATA_PATH creating failed: {}", e.kind())))?;
    }

    if !MEMO_LIST_PATH.exists() {
        fs::create_dir_all(&*MEMO_LIST_PATH)
            .map_err(|e| Error::new(format!("MEMO_LIST_PATH creating failed: {}", e.kind())))?;
    }

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

fn create_new_memo<S: AsRef<str>>(memo_name: S) -> Result<Memo, Error> {
    let memo_name = format!("{}.md", memo_name.as_ref());
    let new_memo_path = MEMO_LIST_PATH.join(memo_name);

    fs::write(&new_memo_path, b"")
        .map_err(|_| Error::new("A file generating failed: the broken name"))?;

    let memo = Memo::new(new_memo_path);

    Ok(memo)
}

fn delete_memo(memo: Memo) -> Result<(), Error> {
    let original_path = &memo.original_path;

    fs::remove_file(original_path)
        .map_err(|e| Error::new(format!("A file cleanup failed: {}", e.kind())))?;

    Ok(())
}

fn fill_stash_with_local(stash: &mut Stash) -> Result<(), Error> {
    let memos = MEMO_LIST_PATH
        .read_dir()
        .map_err(|e| Error::new(format!("Memo files reading failed: {}", e.kind())))?;

    for entry in memos {
        match entry {
            Ok(entry) => stash.push(Memo::with_content(entry.path())?),
            Err(e) => Err(Error::new(format!(
                "A memo file reading failed: {}",
                e.kind()
            )))?,
        }
    }

    Ok(())
}
