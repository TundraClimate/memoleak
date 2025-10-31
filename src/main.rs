use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    self, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus, Stdio};
use std::sync::{Arc, LazyLock, RwLock};
use std::thread;
use viks::{Key, Keymap};

fn main() {
    if let Err(e) = setup() {
        fatal_err("Local setup failed", e);
    }

    let mut stash = Stash::new();

    if let Err(e) = fill_stash_with_local(&mut stash) {
        fatal_err("The memo stash refilling failed", e);
    }
}

fn fatal_err<S: AsRef<str>>(head: S, e: Error) -> ! {
    eprintln!("[ERR] {}", head.as_ref());
    eprintln!("[ERR] {e}");

    process::exit(1)
}

struct Error(String);

impl Error {
    fn new<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref().to_string())
    }

    fn with_cause<S: AsRef<str>, D: Display>(desc: S, cause: D) -> Self {
        Self(format!("{}: {cause}", desc.as_ref()))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

static APP_DATA_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_local_dir()
        .unwrap_or_else(|| {
            fatal_err(
                "APP_DATA_PATH loading failed",
                Error::new("A data dir is not found"),
            )
        })
        .join("memoleak")
});

static MEMO_LIST_PATH: LazyLock<PathBuf> = LazyLock::new(|| APP_DATA_PATH.join("saved_files"));

fn setup() -> Result<(), Error> {
    if !APP_DATA_PATH.exists() {
        fs::create_dir_all(&*APP_DATA_PATH)
            .map_err(|e| Error::with_cause("APP_DATA_PATH creating failed", e.kind()))?;
    }

    if !MEMO_LIST_PATH.exists() {
        fs::create_dir_all(&*MEMO_LIST_PATH)
            .map_err(|e| Error::with_cause("MEMO_LIST_PATH creating failed", e.kind()))?;
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

    fn edit(&self, idx: usize) -> Result<ExitStatus, Error> {
        if idx >= self.stash.len() {
            return Err(Error::new("Index out of bounds"));
        }

        let res = Command::new(option_env!("EDITOR").unwrap_or("vim"))
            .arg(&self.stash[idx].original_path)
            .stderr(Stdio::null())
            .status();

        match res {
            Ok(status) => Ok(status),
            Err(e) => Err(Error::with_cause("$EDITOR executing failed", e.kind())),
        }
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
            Error::with_cause(
                format!(
                    "A file '{}' reading failed",
                    self.original_path.to_string_lossy()
                ),
                e.kind(),
            )
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
    let new_memo_path = MEMO_LIST_PATH.join(&memo_name);

    fs::write(&new_memo_path, b"").map_err(|_| {
        Error::with_cause(
            format!("A memo '{memo_name}' generating failed"),
            "the broken name",
        )
    })?;

    let memo = Memo::new(new_memo_path);

    Ok(memo)
}

fn delete_memo(memo: Memo) -> Result<(), Error> {
    let original_path = &memo.original_path;

    fs::remove_file(original_path).map_err(|e| {
        Error::with_cause(
            format!(
                "A file '{}' cleanup failed",
                original_path.to_string_lossy()
            ),
            e.kind(),
        )
    })?;

    Ok(())
}

fn fill_stash_with_local(stash: &mut Stash) -> Result<(), Error> {
    let memos = MEMO_LIST_PATH
        .read_dir()
        .map_err(|e| Error::with_cause("Memo files reading failed", e.kind()))?;

    for entry in memos {
        match entry {
            Ok(entry) => stash.push(Memo::with_content(entry.path())?),
            Err(e) => Err(Error::with_cause("A memo file reading failed", e.kind()))?,
        }
    }

    Ok(())
}

fn enable_tui() {
    let _ = terminal::enable_raw_mode()
        .and_then(|_| execute!(io::stdout(), DisableLineWrap, EnterAlternateScreen, Hide));
}

fn disable_tui() {
    let _ = terminal::disable_raw_mode()
        .and_then(|_| execute!(io::stdout(), EnableLineWrap, LeaveAlternateScreen, Show));
}

fn setup_tui() -> AppContainer {
    enable_tui();

    let orders: Arc<RwLock<Vec<Order>>> = Arc::new(RwLock::new(vec![]));

    let oc = orders.clone();

    thread::spawn(move || {
        let orders = oc;
        let mut pool: Vec<Key> = vec![];
        let mut maps = HashMap::new();

        maps.insert(Keymap::new("ZZ").unwrap(), Order::Exit);

        let keys = maps.keys().map(|k| k.as_vec()).collect::<Vec<_>>();

        'o: loop {
            if let Ok(Event::Key(ev)) = event::read()
                && let Some(key) = translate_to_key(ev)
            {
                pool.push(key);
            }

            let keymap = Keymap::from(pool.clone());

            if let Some(matched) = maps.get(&keymap) {
                orders.write().unwrap().push(*matched);

                pool.clear();

                continue;
            }

            for key in keys.iter().filter(|k| k.len() > pool.len()) {
                if key[..pool.len()] == pool {
                    continue 'o;
                }
            }

            pool.clear();
        }
    });

    AppContainer::new(orders)
}

fn translate_to_key(key: KeyEvent) -> Option<Key> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut key_str = match key.code {
        KeyCode::Backspace => "BS",
        KeyCode::Tab => "TAB",
        KeyCode::Enter => "ENTER",
        KeyCode::Esc => "ESC",
        KeyCode::Char(' ') => "SPACE",
        KeyCode::Char('!') => "!",
        KeyCode::Char('"') => "\"",
        KeyCode::Char('#') => "#",
        KeyCode::Char('$') => "$",
        KeyCode::Char('%') => "%",
        KeyCode::Char('&') => "&",
        KeyCode::Char('\'') => "'",
        KeyCode::Char('(') => "(",
        KeyCode::Char(')') => ")",
        KeyCode::Char('*') => "*",
        KeyCode::Char('+') => "+",
        KeyCode::Char(',') => ",",
        KeyCode::Char('-') => "-",
        KeyCode::Char('.') => ".",
        KeyCode::Char('/') => "/",
        KeyCode::Char('0') => "0",
        KeyCode::Char('1') => "1",
        KeyCode::Char('2') => "2",
        KeyCode::Char('3') => "3",
        KeyCode::Char('4') => "4",
        KeyCode::Char('5') => "5",
        KeyCode::Char('6') => "6",
        KeyCode::Char('7') => "7",
        KeyCode::Char('8') => "8",
        KeyCode::Char('9') => "9",
        KeyCode::Char(':') => ":",
        KeyCode::Char(';') => ";",
        KeyCode::Char('<') => "lt",
        KeyCode::Char('=') => "=",
        KeyCode::Char('>') => ">",
        KeyCode::Char('?') => "?",
        KeyCode::Char('@') => "@",
        KeyCode::Char('a') => "a",
        KeyCode::Char('b') => "b",
        KeyCode::Char('c') => "c",
        KeyCode::Char('d') => "d",
        KeyCode::Char('e') => "e",
        KeyCode::Char('f') => "f",
        KeyCode::Char('g') => "g",
        KeyCode::Char('h') => "h",
        KeyCode::Char('i') => "i",
        KeyCode::Char('j') => "j",
        KeyCode::Char('k') => "k",
        KeyCode::Char('l') => "l",
        KeyCode::Char('m') => "m",
        KeyCode::Char('n') => "n",
        KeyCode::Char('o') => "o",
        KeyCode::Char('p') => "p",
        KeyCode::Char('q') => "q",
        KeyCode::Char('r') => "r",
        KeyCode::Char('s') => "s",
        KeyCode::Char('t') => "t",
        KeyCode::Char('u') => "u",
        KeyCode::Char('v') => "v",
        KeyCode::Char('w') => "w",
        KeyCode::Char('x') => "x",
        KeyCode::Char('y') => "y",
        KeyCode::Char('z') => "z",
        KeyCode::Char('A') => "A",
        KeyCode::Char('B') => "B",
        KeyCode::Char('C') => "C",
        KeyCode::Char('D') => "D",
        KeyCode::Char('E') => "E",
        KeyCode::Char('F') => "F",
        KeyCode::Char('G') => "G",
        KeyCode::Char('H') => "H",
        KeyCode::Char('I') => "I",
        KeyCode::Char('J') => "J",
        KeyCode::Char('K') => "K",
        KeyCode::Char('L') => "L",
        KeyCode::Char('M') => "M",
        KeyCode::Char('N') => "N",
        KeyCode::Char('O') => "O",
        KeyCode::Char('P') => "P",
        KeyCode::Char('Q') => "Q",
        KeyCode::Char('R') => "R",
        KeyCode::Char('S') => "S",
        KeyCode::Char('T') => "T",
        KeyCode::Char('U') => "U",
        KeyCode::Char('V') => "V",
        KeyCode::Char('W') => "W",
        KeyCode::Char('X') => "X",
        KeyCode::Char('Y') => "Y",
        KeyCode::Char('Z') => "Z",
        KeyCode::Char('[') => "[",
        KeyCode::Char('\\') => "\\",
        KeyCode::Char(']') => "]",
        KeyCode::Char('^') => "^",
        KeyCode::Char('_') => "_",
        KeyCode::Char('`') => "`",
        KeyCode::Char('{') => "{",
        KeyCode::Char('|') => "|",
        KeyCode::Char('}') => "}",
        KeyCode::Char('~') => "~",
        KeyCode::Delete => "DEL",
        _ => return None,
    }
    .to_string();

    let is_big_alpha = key_str.len() == 1 && matches!(key_str.chars().next(), Some('A'..='Z'));

    if !is_big_alpha && key.modifiers.contains(KeyModifiers::SHIFT) {
        key_str = format!("s-{key_str}");
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        key_str = format!("a-{key_str}");
    } else if key.modifiers.contains(KeyModifiers::CONTROL) {
        key_str = format!("c-{key_str}");
    }

    let key_str = if key_str.len() > 1 {
        format!("<{key_str}>")
    } else {
        key_str
    };

    Key::new(&key_str).ok()
}

struct AppContainer {
    orders: Arc<RwLock<Vec<Order>>>,
}

impl AppContainer {
    fn new(orders: Arc<RwLock<Vec<Order>>>) -> Self {
        Self { orders }
    }
}

#[derive(Clone, Copy)]
enum Order {
    Exit,
}
