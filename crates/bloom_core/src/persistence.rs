//! Save files: versioned, checksummed, written atomically with a backup.
//!
//! File format (text, human-inspectable):
//!
//! ```text
//! BLOOM-AND-GATHER-SAVE v1 7f3a9c0e12ab45cd
//! ( ...GameState as RON... )
//! ```
//!
//! The header carries the format version and an FNV-1a 64 checksum of the
//! payload, so truncated or edited files are detected. Writes go to a
//! temporary file that is flushed to disk and then renamed into place; the
//! previous save is kept as `save.bak`, and loading falls back to it if the
//! main file is missing or damaged. Harvested flowers can therefore never
//! "come back" because of a crash mid-save.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::game::GameState;

pub const SAVE_VERSION: u32 = 1;
const MAGIC: &str = "BLOOM-AND-GATHER-SAVE";
const SAVE_FILE: &str = "save.ron";
const BACKUP_FILE: &str = "save.bak";

#[derive(Debug, Clone, PartialEq)]
pub enum SaveError {
    Io(String),
    /// Not a save file, or the header is damaged.
    BadHeader,
    /// Payload doesn't match its checksum (truncated or edited).
    Corrupt,
    /// Written by a newer version of the game.
    TooNew {
        version: u32,
    },
    /// Payload couldn't be parsed.
    Parse(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "could not access the save: {e}"),
            SaveError::BadHeader => write!(f, "not a Bloom & Gather save file"),
            SaveError::Corrupt => write!(f, "the save file is damaged"),
            SaveError::TooNew { version } => {
                write!(
                    f,
                    "the save is from a newer version of the game (format v{version})"
                )
            }
            SaveError::Parse(e) => write!(f, "the save could not be read: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// FNV-1a 64-bit: tiny, dependency-free, plenty to detect damage.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

pub fn encode(state: &GameState) -> String {
    let payload = ron::ser::to_string_pretty(state, ron::ser::PrettyConfig::default())
        .expect("GameState is always serialisable");
    format!(
        "{MAGIC} v{SAVE_VERSION} {:016x}\n{payload}",
        checksum(payload.as_bytes())
    )
}

pub fn decode(text: &str) -> Result<GameState, SaveError> {
    let (header, payload) = text.split_once('\n').ok_or(SaveError::BadHeader)?;
    let mut parts = header.split(' ');
    if parts.next() != Some(MAGIC) {
        return Err(SaveError::BadHeader);
    }
    let version = parts
        .next()
        .and_then(|v| v.strip_prefix('v'))
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or(SaveError::BadHeader)?;
    let sum = parts
        .next()
        .and_then(|s| u64::from_str_radix(s, 16).ok())
        .ok_or(SaveError::BadHeader)?;
    if checksum(payload.as_bytes()) != sum {
        return Err(SaveError::Corrupt);
    }
    if version > SAVE_VERSION {
        return Err(SaveError::TooNew { version });
    }
    let payload = migrate(version, payload)?;
    ron::from_str(&payload).map_err(|e| SaveError::Parse(e.to_string()))
}

/// Upgrade an older payload to the current format, one version at a time.
/// Each future format change adds a step here plus a fixture test.
fn migrate(version: u32, payload: &str) -> Result<String, SaveError> {
    match version {
        SAVE_VERSION => Ok(payload.to_owned()),
        other => Err(SaveError::Parse(format!(
            "unsupported save format v{other}"
        ))),
    }
}

/// Write `contents` to `path` atomically: temp file → flush to disk →
/// rename. Readers see either the old or the new file, never a partial one.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSource {
    Primary,
    /// The main save was missing or damaged; the previous save was used.
    Backup,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub state: GameState,
    pub source: LoadSource,
    /// Why the primary file was skipped, if it was.
    pub primary_error: Option<SaveError>,
}

#[derive(Debug, Clone, Copy)]
pub struct SaveReport {
    pub bytes: usize,
    pub millis: f64,
}

/// The save slot on disk.
#[derive(Debug, Clone)]
pub struct SaveStore {
    dir: PathBuf,
}

impl SaveStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The platform's per-user data folder, overridable with
    /// `BLOOM_SAVE_DIR` (used by tests and the smoke test).
    pub fn default_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os("BLOOM_SAVE_DIR") {
            return PathBuf::from(dir);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Bloom and Gather")
        } else if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or(home)
                .join("Bloom and Gather")
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share"))
                .join("bloom-and-gather")
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn primary(&self) -> PathBuf {
        self.dir.join(SAVE_FILE)
    }

    fn backup(&self) -> PathBuf {
        self.dir.join(BACKUP_FILE)
    }

    pub fn exists(&self) -> bool {
        self.primary().exists() || self.backup().exists()
    }

    /// Write the state, keeping the previous save as the backup.
    pub fn save(&self, state: &GameState) -> Result<SaveReport, SaveError> {
        let started = Instant::now();
        let text = encode(state);
        let io = |e: std::io::Error| SaveError::Io(e.to_string());
        fs::create_dir_all(&self.dir).map_err(io)?;
        let tmp = self.primary().with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(io)?;
            file.write_all(text.as_bytes()).map_err(io)?;
            file.sync_all().map_err(io)?;
        }
        // Only a primary that still decodes becomes the backup, so a damaged
        // file can never push out the last good one.
        if let Ok(previous) = fs::read_to_string(self.primary()) {
            if decode(&previous).is_ok() {
                fs::rename(self.primary(), self.backup()).map_err(io)?;
            }
        }
        fs::rename(&tmp, self.primary()).map_err(io)?;
        Ok(SaveReport {
            bytes: text.len(),
            millis: started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Load the save, falling back to the backup. `Ok(None)` if there is no
    /// save at all.
    pub fn load(&self) -> Result<Option<Loaded>, SaveError> {
        let read = |p: PathBuf| fs::read_to_string(p).map_err(|e| SaveError::Io(e.to_string()));
        let primary = if self.primary().exists() {
            Some(read(self.primary()).and_then(|t| decode(&t)))
        } else {
            None
        };
        match primary {
            Some(Ok(state)) => {
                return Ok(Some(Loaded {
                    state,
                    source: LoadSource::Primary,
                    primary_error: None,
                }))
            }
            Some(Err(SaveError::TooNew { version })) => return Err(SaveError::TooNew { version }),
            _ => {}
        }
        let primary_error = primary.and_then(Result::err);
        if !self.backup().exists() {
            return match primary_error {
                Some(e) => Err(e),
                None => Ok(None),
            };
        }
        let state = read(self.backup()).and_then(|t| decode(&t))?;
        Ok(Some(Loaded {
            state,
            source: LoadSource::Backup,
            primary_error,
        }))
    }

    /// Rename an unreadable main save to `save.unreadable-<unix secs>` so a
    /// fresh game can't overwrite it. Returns the new path, or `None` if
    /// there was no main save.
    pub fn quarantine(&self) -> Result<Option<PathBuf>, SaveError> {
        if !self.primary().exists() {
            return Ok(None);
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = self.dir.join(format!("save.unreadable-{secs}"));
        fs::rename(self.primary(), &target).map_err(|e| SaveError::Io(e.to_string()))?;
        Ok(Some(target))
    }

    /// Remove the save and its backup (starting over).
    pub fn clear(&self) -> Result<(), SaveError> {
        for path in [self.primary(), self.backup()] {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| SaveError::Io(e.to_string()))?;
            }
        }
        Ok(())
    }
}
