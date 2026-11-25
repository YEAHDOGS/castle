//! Where the vault is kept, when the box is allowed to choose.
//!
//! A box is told its vault when it starts (--data). Started with --places and --state as
//! well, it can be moved from the app: `--places` names a folder whose subfolders are the
//! places a vault may live (in Docker, each drive is mounted as one: /drives/big), and
//! `--state` names a small folder outside every vault where the choice is remembered.
//!
//! Choosing a place does three things: makes `<place>/Castle` there, copies the box's token
//! and settings into it if it is new (so every connected device stays connected), and
//! writes the choice down. The box then stops, and whatever runs it starts it again on the
//! new vault. Files are not moved: the old vault stays where it is, whole, and choosing it
//! again brings it back.
//!
//! Only the folders under --places can be chosen, and only by name, so the app can never
//! point the box at an arbitrary folder of the computer.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The vault's folder inside a place, so a drive can hold other things beside it.
const VAULT_FOLDER: &str = "Castle";
const CHOICE_FILE: &str = "vault";
const META_DIR: &str = ".castle";
/// What is carried into a new vault, so devices and settings survive the move.
const CARRIED: [&str; 2] = ["token", "settings.json"];
const NOT_IN_A_NAME: [char; 3] = ['/', '\\', ':'];

pub struct Places {
    /// The folder whose subfolders can be chosen.
    pub places: PathBuf,
    /// Where the choice is written.
    pub state: PathBuf,
    /// The vault the box was started with (--data): the place called "".
    pub default: PathBuf,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// What to choose it by: the folder's name, or "" for the vault the box was started with.
    pub id: String,
    pub name: String,
    /// Where the vault is, or would be.
    pub path: String,
    pub free: u64,
    pub total: u64,
    pub current: bool,
    /// A Castle vault is already there: choosing it opens what it holds.
    pub has_vault: bool,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 100 && !name.starts_with('.') && !name.contains(NOT_IN_A_NAME)
}

impl Places {
    fn vault_of(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() {
            return Ok(self.default.clone());
        }
        if !valid_name(id) || !self.places.join(id).is_dir() {
            return Err("that place is not one this box can use".into());
        }
        Ok(self.places.join(id).join(VAULT_FOLDER))
    }

    /// The vault to open at start: the one chosen from the app, if it is still there.
    /// A chosen drive that is missing is reported (second value) and the box falls back to
    /// the vault it was started with, rather than making an empty vault where the drive
    /// should have been.
    pub fn at_start(&self) -> (PathBuf, Option<String>) {
        let chosen = std::fs::read_to_string(self.state.join(CHOICE_FILE)).unwrap_or_default();
        let id = chosen.trim();
        if id.is_empty() {
            return (self.default.clone(), None);
        }
        match self.vault_of(id) {
            Ok(vault) if vault.join(META_DIR).is_dir() => (vault, None),
            _ => (self.default.clone(), Some(id.to_string())),
        }
    }

    /// Every place the vault can be, the one in use marked.
    pub fn list(&self, current: &Path) -> Vec<Place> {
        let same = |path: &Path| path.canonicalize().is_ok_and(|real| real == current);
        let describe = |id: &str, name: String, vault: PathBuf, on: &Path| Place {
            id: id.to_string(),
            name,
            path: vault.display().to_string(),
            free: fs4::available_space(on).unwrap_or(0),
            total: fs4::total_space(on).unwrap_or(0),
            current: same(&vault),
            has_vault: vault.join(META_DIR).is_dir(),
        };
        let mut found = vec![describe("", "Where the box started".into(), self.default.clone(), &self.default)];
        let mut names: Vec<String> = std::fs::read_dir(&self.places)
            .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|name| valid_name(name)).collect())
            .unwrap_or_default();
        names.sort_by_key(|name| name.to_lowercase());
        for name in names {
            let folder = self.places.join(&name);
            found.push(describe(&name, name.clone(), folder.join(VAULT_FOLDER), &folder));
        }
        found
    }

    /// Makes `id` the vault from the next start on. `current` is the vault in use now.
    pub fn choose(&self, id: &str, current: &Path) -> Result<PathBuf, String> {
        let vault = self.vault_of(id)?;
        let meta = vault.join(META_DIR);
        let is_new = !meta.is_dir();
        std::fs::create_dir_all(&meta).map_err(|_| "that place cannot be written to".to_string())?;
        if is_new {
            for name in CARRIED {
                let from = current.join(META_DIR).join(name);
                if from.is_file() {
                    std::fs::copy(&from, meta.join(name)).map_err(|_| "that place cannot be written to".to_string())?;
                }
            }
        }
        std::fs::create_dir_all(&self.state).map_err(|_| "the choice could not be saved".to_string())?;
        std::fs::write(self.state.join(CHOICE_FILE), id).map_err(|_| "the choice could not be saved".to_string())?;
        Ok(vault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chosen_place_is_opened_at_the_next_start_and_keeps_the_token() {
        let base = std::env::temp_dir().join(format!("castle-places-{}", crate::random_hex(6)));
        let places = Places { places: base.join("drives"), state: base.join("state"), default: base.join("vault") };
        std::fs::create_dir_all(places.default.join(META_DIR)).unwrap();
        std::fs::write(places.default.join(META_DIR).join("token"), "secret").unwrap();
        std::fs::create_dir_all(places.places.join("big")).unwrap();
        std::fs::create_dir_all(places.places.join(".hidden")).unwrap();
        let current = places.default.canonicalize().unwrap();

        assert_eq!(places.at_start(), (places.default.clone(), None));
        let listed = places.list(&current);
        assert_eq!(listed.iter().map(|p| (p.id.as_str(), p.current, p.has_vault)).collect::<Vec<_>>(), [("", true, true), ("big", false, false)]);

        for bad in ["nowhere", "..", "../vault", "big/..", ".hidden"] {
            assert!(places.choose(bad, &current).is_err(), "{bad}");
        }
        let vault = places.choose("big", &current).unwrap();
        assert_eq!(vault, places.places.join("big").join("Castle"));
        assert_eq!(std::fs::read_to_string(vault.join(META_DIR).join("token")).unwrap(), "secret");
        assert_eq!(places.at_start(), (vault.clone(), None));

        // Choosing it again later does not overwrite what that vault has become.
        std::fs::write(vault.join(META_DIR).join("token"), "rotated").unwrap();
        places.choose("big", &current).unwrap();
        assert_eq!(std::fs::read_to_string(vault.join(META_DIR).join("token")).unwrap(), "rotated");

        // The drive is unplugged: the box says so and opens the vault it was started with.
        std::fs::rename(places.places.join("big"), places.places.join("elsewhere")).unwrap();
        assert_eq!(places.at_start(), (places.default.clone(), Some("big".into())));
        places.choose("", &current).unwrap();
        assert_eq!(places.at_start(), (places.default.clone(), None));
        let _ = std::fs::remove_dir_all(base);
    }
}
