//! Continue-card rename apply (feature 37): disk rename, store rekey, strip retarget.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::SiblingSearchState;

#[path = "card_rename.rs"]
mod card_rename;
pub(crate) use card_rename::prompt_card_rename;

impl SiblingSearchState {
    /// Rename on disk + store, then refresh the strip.
    pub(crate) fn rename_card_file(self: &Rc<Self>, path: &Path, stem: &str) -> Result<(), String> {
        let dest = rename_dest(path, stem)?;
        if dest == path {
            return Ok(());
        }
        ensure_dest_free(path, &dest)?;
        rename_on_disk_and_store(path, &dest)?;
        crate::db::record_history(&dest);
        self.note_path_renamed(path, &dest);
        Ok(())
    }

    fn note_path_renamed(self: &Rc<Self>, from: &Path, to: &Path) {
        if self.lucky.is_active() {
            self.lucky.deactivate();
        }
        retarget_hit_cache(self, from, to);
        self.retarget_index_path(from, to);
        self.catalog.refresh_progress();
        self.clear_hits_paint();
        let Some(c) = self.ctx.borrow().as_ref().and_then(|w| w.upgrade()) else {
            eprintln!("[rhino] rename: strip refresh skipped (no context)");
            return;
        };
        c.apply_strip();
    }

    fn retarget_index_path(&self, from: &Path, to: &Path) {
        let mut index = self.catalog.index_mut();
        for e in index.iter_mut() {
            if crate::video_ext::paths_same_file(&e.path, from) {
                e.name_lower = super::super::file_name_lower(to);
                e.path = to.to_path_buf();
            }
        }
        let snap: Vec<_> = index
            .iter()
            .map(|e| super::super::FilterRow {
                path: e.path.clone(),
                name_lower: e.name_lower.clone(),
            })
            .collect();
        drop(index);
        *self.catalog.filter_snap.borrow_mut() = Some(std::sync::Arc::new(snap));
    }
}

fn retarget_hit_cache(state: &SiblingSearchState, from: &Path, to: &Path) {
    if let Some(c) = state.hit_cache.borrow_mut().as_mut() {
        for p in &mut c.hits {
            if crate::video_ext::paths_same_file(p, from) {
                *p = to.to_path_buf();
            }
        }
    }
}

fn ensure_dest_free(path: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() && !crate::video_ext::paths_same_file(path, dest) {
        return Err("A file with that name already exists.".into());
    }
    Ok(())
}

fn rename_on_disk_and_store(path: &Path, dest: &Path) -> Result<(), String> {
    std::fs::rename(path, dest).map_err(|e| format!("Could not rename the file ({e})."))?;
    if let Err(e) = crate::db::rekey_renamed_path(path, dest) {
        return undo_disk_rename(path, dest, e);
    }
    Ok(())
}

fn undo_disk_rename(from: &Path, to: &Path, store_err: String) -> Result<(), String> {
    if let Err(back) = std::fs::rename(to, from) {
        eprintln!(
            "[rhino] rename: store failed and undo failed from={} to={}: {store_err}; undo: {back}",
            from.display(),
            to.display()
        );
        return Err(format!(
            "Renamed on disk but could not update the library ({store_err})."
        ));
    }
    eprintln!(
        "[rhino] rename: store failed, restored path={}",
        from.display()
    );
    Err(store_err)
}

fn rename_dest(path: &Path, stem: &str) -> Result<PathBuf, String> {
    let stem = validated_stem(stem)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Could not resolve the file folder.".to_string())?;
    Ok(join_stem_ext(parent, stem, path.extension()))
}

fn validated_stem(stem: &str) -> Result<&str, String> {
    let stem = stem.trim();
    if stem.is_empty() || stem == "." || stem == ".." {
        return Err("Enter a file name.".into());
    }
    if stem.contains('/') || stem.contains('\\') {
        return Err("The name cannot contain path separators.".into());
    }
    Ok(stem)
}

fn join_stem_ext(parent: &Path, stem: &str, ext: Option<&std::ffi::OsStr>) -> PathBuf {
    let mut name = OsString::from(stem);
    if let Some(ext) = ext {
        name.push(".");
        name.push(ext);
    }
    parent.join(name)
}

#[cfg(test)]
mod tests {
    use super::rename_dest;
    use std::path::Path;

    #[test]
    fn keeps_extension() {
        let d = rename_dest(Path::new("/v/Show.S01E02.mkv"), "Better Name").unwrap();
        assert_eq!(d, Path::new("/v/Better Name.mkv"));
    }

    #[test]
    fn rejects_separator() {
        assert!(rename_dest(Path::new("/v/a.mkv"), "a/b").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(rename_dest(Path::new("/v/a.mkv"), "  ").is_err());
    }
}
