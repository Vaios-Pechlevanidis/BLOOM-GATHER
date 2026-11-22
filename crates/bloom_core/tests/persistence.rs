//! Save files: round trips, damage detection, backups and atomicity.

use std::path::PathBuf;
use std::sync::Arc;

use bloom_core::persistence::{decode, encode, LoadSource, SaveError, SaveStore};
use bloom_core::{Game, GameData};
use glam::Vec3;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("bloom-test-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn played_game(flowers: &[u32]) -> Game {
    let mut game = Game::new(Arc::new(GameData::builtin().unwrap()));
    for &i in flowers {
        let (c, _) = game.region(0).field.pick_sphere(i);
        game.tick(5.0);
        game.harvest(0, i, c + Vec3::Y).unwrap();
    }
    game
}

#[test]
fn encode_decode_round_trip() {
    let game = played_game(&[1, 2, 3]);
    let text = encode(game.state());
    assert!(text.starts_with("BLOOM-AND-GATHER-SAVE v1 "));
    assert_eq!(&decode(&text).unwrap(), game.state());
}

#[test]
fn damage_is_detected() {
    let game = played_game(&[1]);
    let text = encode(game.state());
    let edited = text.replacen("coins: ", "coins: 9999", 1);
    assert_ne!(edited, text);
    assert_eq!(decode(&edited), Err(SaveError::Corrupt));
    assert_eq!(decode(&text[..text.len() / 2]), Err(SaveError::Corrupt));
    assert_eq!(decode("hello\n()"), Err(SaveError::BadHeader));
    assert_eq!(decode(""), Err(SaveError::BadHeader));
}

#[test]
fn newer_format_is_refused_not_overwritten() {
    let game = played_game(&[]);
    let text = encode(game.state()).replacen(" v1 ", " v99 ", 1);
    assert_eq!(decode(&text), Err(SaveError::TooNew { version: 99 }));
}

#[test]
fn store_saves_loads_and_keeps_a_backup() {
    let dir = temp_dir("store");
    let store = SaveStore::new(&dir);
    assert!(!store.exists());
    assert!(store.load().unwrap().is_none());

    let first = played_game(&[4]);
    store.save(first.state()).unwrap();
    let second = played_game(&[4, 5, 6]);
    let report = store.save(second.state()).unwrap();
    assert!(report.bytes > 0);

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.source, LoadSource::Primary);
    assert_eq!(&loaded.state, second.state());
    assert!(dir.join("save.bak").exists());
    assert!(!dir.join("save.tmp").exists(), "no temp file left behind");
}

#[test]
fn damaged_save_falls_back_to_backup() {
    let dir = temp_dir("fallback");
    let store = SaveStore::new(&dir);
    let older = played_game(&[7]);
    store.save(older.state()).unwrap();
    let newer = played_game(&[7, 8]);
    store.save(newer.state()).unwrap();
    // Simulate a crash that truncated the main file.
    let text = std::fs::read_to_string(dir.join("save.ron")).unwrap();
    std::fs::write(dir.join("save.ron"), &text[..text.len() - 20]).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.source, LoadSource::Backup);
    assert_eq!(loaded.primary_error, Some(SaveError::Corrupt));
    assert_eq!(&loaded.state, older.state());
    // Harvested flowers in the backup stay harvested.
    let restored = Game::from_state(Arc::new(GameData::builtin().unwrap()), loaded.state).unwrap();
    assert!(restored.is_harvested(0, 7));
}

#[test]
fn damaged_primary_never_replaces_the_good_backup() {
    let dir = temp_dir("keepgood");
    let store = SaveStore::new(&dir);
    let good = played_game(&[1]);
    store.save(good.state()).unwrap();
    store.save(good.state()).unwrap(); // backup = good
    std::fs::write(dir.join("save.ron"), "garbage").unwrap();
    let next = played_game(&[1, 2]);
    store.save(next.state()).unwrap();
    // The garbage primary was discarded, the good backup kept.
    let backup = std::fs::read_to_string(dir.join("save.bak")).unwrap();
    assert_eq!(&decode(&backup).unwrap(), good.state());
}

#[test]
fn clear_removes_the_slot() {
    let dir = temp_dir("clear");
    let store = SaveStore::new(&dir);
    store.save(played_game(&[]).state()).unwrap();
    store.save(played_game(&[]).state()).unwrap();
    assert!(store.exists());
    store.clear().unwrap();
    assert!(!store.exists());
    assert!(store.load().unwrap().is_none());
}

#[test]
fn quarantine_keeps_unreadable_saves() {
    let dir = temp_dir("quarantine");
    let store = SaveStore::new(&dir);
    assert_eq!(store.quarantine(), Ok(None));
    std::fs::write(dir.join("save.ron"), "garbage").unwrap();
    let kept = store.quarantine().unwrap().unwrap();
    assert!(kept.exists());
    assert_eq!(std::fs::read_to_string(kept).unwrap(), "garbage");
    assert!(!store.exists());
}
