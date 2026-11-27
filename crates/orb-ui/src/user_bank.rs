//! Shared AcidSynth sound library, stored beside V2's UI preferences.
use orb_engine::studio::acid_synth::{Patch, Slot};
use serde::{Deserialize, Serialize};
use std::{fs, io::{self, Write}, path::{Path, PathBuf}};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Sound { patch: Patch, macros: [f32; 4], preset: u8 }

impl Sound {
    pub(crate) fn capture(slot: &Slot) -> Self {
        Self { patch: slot.patch, macros: slot.macros, preset: slot.preset }
    }

    pub(crate) fn recall(self, slot: &mut Slot, name: &str) {
        slot.patch = self.patch;
        slot.macros = self.macros;
        slot.preset = self.preset;
        slot.sanitize_sound();
        slot.user_preset = name.to_owned();
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Entry { pub name: String, pub sound: Sound }

#[derive(Serialize, Deserialize)]
struct FileBank { version: u32, sounds: Vec<Entry> }

fn name(raw: &str) -> Result<&str, String> {
    let value = raw.trim();
    if value.is_empty() || value.chars().count() > 64 || value.chars().any(char::is_control) {
        return Err("Choose a name of 1–64 printable characters.".into());
    }
    Ok(value)
}

fn read(path: &Path) -> Result<Vec<Entry>, String> {
    let raw = match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        other => other.map_err(|e| e.to_string())?,
    };
    let bank: FileBank = toml::from_str(&raw).map_err(|e| format!("Cannot read USER bank: {e}"))?;
    if bank.version != 1 { return Err(format!("Unsupported USER bank version {}.", bank.version)); }
    let mut keys = std::collections::HashSet::new();
    for entry in &bank.sounds {
        if name(&entry.name)? != entry.name || !keys.insert(entry.name.to_lowercase()) {
            return Err("USER bank contains invalid or duplicate names.".into());
        }
    }
    Ok(bank.sounds)
}

#[derive(Default)]
pub(crate) struct UserBank {
    path: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub names: [String; 2],
    pub confirm: [Option<Entry>; 2],
    pub error: Option<String>,
}

impl UserBank {
    pub fn configure(&mut self, config: Option<&Path>) {
        let Some(config) = config else { return };
        if self.path.as_deref().and_then(Path::parent) == config.parent() && self.path.is_some() { return; }
        self.path = Some(config.with_file_name("acid-synth-user.toml"));
        self.refresh();
    }

    pub fn refresh(&mut self) {
        let Some(path) = self.path.as_deref() else { return; };
        match read(path) {
            Ok(entries) => { self.entries = entries; self.error = None; }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn save(&mut self, row: usize, slot: &mut Slot, replace: bool) {
        let result = self.write(row, Sound::capture(slot), replace);
        match result {
            Ok(Some(entry)) => { self.confirm[row] = Some(entry); self.error = None; }
            Ok(None) => {
                slot.user_preset = self.names[row].trim().to_string();
                self.confirm[row] = None;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    // Re-read under the lock, including when confirming an overwrite after another instance saved.
    fn write(&mut self, row: usize, sound: Sound, replace: bool) -> Result<Option<Entry>, String> {
        let desired = name(&self.names[row])?.to_owned();
        let path = self.path.as_ref().ok_or("Application preferences folder unavailable.")?;
        let parent = path.parent().ok_or("USER bank folder unavailable.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let lock = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .open(path.with_extension("lock")).map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|e| format!("USER bank unavailable. Try again. {e}"))?;
        let mut entries = read(path)?;
        let entry = Entry { name: desired.clone(), sound };
        if let Some(old) = entries.iter_mut().find(|e| e.name.to_lowercase() == desired.to_lowercase()) {
            if !replace || self.confirm[row].as_ref() != Some(&*old) { return Ok(Some(old.clone())); }
            *old = entry;
        } else { entries.push(entry); }
        entries.sort_by_cached_key(|e| e.name.to_lowercase());
        let bank = FileBank { version: 1, sounds: entries };
        let data = toml::to_string(&bank).map_err(|e| e.to_string())?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        tmp.write_all(data.as_bytes()).and_then(|_| tmp.as_file().sync_all()).map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.to_string())?;
        self.entries = bank.sounds;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_bank_survives_restart_and_confirms_replacement_of_latest_revision() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("ui.toml");
        let open = || { let mut b = UserBank::default(); b.configure(Some(&config)); b };
        let mut a = open();
        let mut b = open();
        let mut slot = Slot::default();
        a.names[0] = " Aube ".into();
        a.save(0, &mut slot, false);
        assert_eq!(slot.user_preset, "Aube");
        b.names[1] = "Orage".into();
        b.save(1, &mut slot, false);
        a.names[0] = "aube".into();
        slot.patch.cutoff = 0.17;
        a.save(0, &mut slot, false);
        assert!(a.confirm[0].is_some());
        a.save(0, &mut slot, true);
        let reopened = open();
        assert_eq!(reopened.entries.len(), 2);
        let mut other = Slot::default();
        other.enabled = true;
        let e = &reopened.entries[0];
        e.sound.recall(&mut other, &e.name);
        assert_eq!(other.patch, slot.patch);
        assert!(other.enabled);
        assert_eq!(other.user_preset, "aube");
        other.select_factory_preset(0);
        assert!(other.user_preset.is_empty());
        fs::write(dir.path().join("acid-synth-user.toml"), "broken").unwrap();
        a.save(0, &mut slot, true);
        assert!(a.error.is_some());
        assert_eq!(fs::read_to_string(dir.path().join("acid-synth-user.toml")).unwrap(), "broken");
    }
}
