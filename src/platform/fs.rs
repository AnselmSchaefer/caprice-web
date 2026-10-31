//! Files. On the desktop these are the system's files, as Caprice always had them. A page has no
//! files (`std::fs` only fails there), so the browser keeps its own: whole files by path, in memory,
//! read from IndexedDB before the app starts and written back to it after each change. Everything
//! that keeps files (versions, the theme, the word goal, the stories last opened) works the same
//! on both, by path. Two folders are special in the browser: a file written under `/downloads`
//! is given to the writer as a download, and one under `/uploads` (a file they gave) is not kept.
//!
//! In the browser only the page's main thread has the files: a worker sees none, rather than
//! share a lock with it (R31).

use std::io;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
mod native {
    #![allow(clippy::disallowed_methods)]

    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        std::fs::write(path, bytes)
    }

    pub fn create_dir_all(dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }

    pub fn remove_file(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    pub fn read_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
        std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect()
    }

    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    fn secs(t: io::Result<SystemTime>) -> Option<u64> {
        t.ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
    }

    pub fn modified(path: &Path) -> Option<u64> {
        secs(std::fs::metadata(path).and_then(|m| m.modified()))
    }

    pub fn made(path: &Path) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        secs(meta.created()).or_else(|| secs(meta.modified()))
    }

    pub fn canonicalize(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native as sys;

/// Files held in memory, by path: the browser's files. Folders are not kept; a folder is there
/// while a file is in it.
#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Default)]
pub struct Memory {
    files: std::collections::BTreeMap<PathBuf, (Vec<u8>, u64)>,
}

#[cfg(any(test, target_arch = "wasm32"))]
impl Memory {
    fn missing(path: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, format!("{} is not there", path.display()))
    }

    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.files.get(path).map(|(bytes, _)| bytes.clone()).ok_or_else(|| Self::missing(path))
    }

    /// Write `bytes` to `path` at `now` (seconds since 1970).
    pub fn write(&mut self, path: &Path, bytes: &[u8], now: u64) {
        self.files.insert(path.to_owned(), (bytes.to_vec(), now));
    }

    pub fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.files.remove(path).map(|_| ()).ok_or_else(|| Self::missing(path))
    }

    /// What is in the folder `dir`: its files, and the folders its deeper files are in, once each.
    pub fn read_dir(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        let mut found: Vec<PathBuf> = Vec::new();
        for path in self.files.keys() {
            if let Ok(rest) = path.strip_prefix(dir)
                && let Some(first) = rest.components().next()
            {
                let entry = dir.join(first);
                if found.last() != Some(&entry) {
                    found.push(entry);
                }
            }
        }
        if found.is_empty() { Err(Self::missing(dir)) } else { Ok(found) }
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path) || self.files.keys().any(|p| p.starts_with(path))
    }

    pub fn modified(&self, path: &Path) -> Option<u64> {
        self.files.get(path).map(|(_, at)| *at)
    }

    #[cfg(test)]
    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

    use wasm_bindgen::prelude::*;

    use super::Memory;

    // IndexedDB holds a copy of each file, put and deleted as the app writes. What is handed to
    // it is copied out of the app's memory first: that memory is shared with the workers, and
    // IndexedDB will not store shared memory.
    #[wasm_bindgen(inline_js = r#"
        const STORE = "files";
        let db = null;
        export async function files_load() {
            db = await new Promise((done, failed) => {
                const r = indexedDB.open("caprice", 1);
                r.onupgradeneeded = () => r.result.createObjectStore(STORE);
                r.onsuccess = () => done(r.result);
                r.onerror = () => failed(r.error);
            });
            // Ask the browser not to clear the stories when it runs short of room.
            if (navigator.storage && navigator.storage.persist) navigator.storage.persist();
            return await new Promise((done, failed) => {
                const found = [];
                const req = db.transaction(STORE, "readonly").objectStore(STORE).openCursor();
                req.onsuccess = () => {
                    const c = req.result;
                    if (!c) return done(found);
                    found.push([c.key, c.value.bytes, c.value.modified]);
                    c.continue();
                };
                req.onerror = () => failed(req.error);
            });
        }
        export function files_put(path, bytes, modified) {
            if (db) db.transaction(STORE, "readwrite").objectStore(STORE).put({ bytes: new Uint8Array(bytes), modified }, path);
        }
        export function files_delete(path) {
            if (db) db.transaction(STORE, "readwrite").objectStore(STORE).delete(path);
        }
    "#)]
    extern "C" {
        #[wasm_bindgen(catch)]
        async fn files_load() -> Result<JsValue, JsValue>;
        fn files_put(path: &str, bytes: &[u8], modified: f64);
        fn files_delete(path: &str);
    }

    thread_local! {
        static FILES: RefCell<Memory> = RefCell::new(Memory::default());
    }

    /// Read every file from IndexedDB: before the app starts, so it never waits for one.
    pub async fn load() -> Result<usize, String> {
        let found = files_load().await.map_err(|e| format!("{e:?}"))?;
        let found = js_sys::Array::from(&found);
        FILES.with_borrow_mut(|files| {
            for entry in found.iter() {
                let entry = js_sys::Array::from(&entry);
                let path = entry.get(0).as_string().unwrap_or_default();
                let bytes = js_sys::Uint8Array::new(&entry.get(1)).to_vec();
                let at = entry.get(2).as_f64().unwrap_or(0.0) as u64;
                files.write(Path::new(&path), &bytes, at);
            }
        });
        Ok(found.length() as usize)
    }

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        FILES.with_borrow(|f| f.read(path))
    }

    /// Written to the downloads, a file is given to the writer, not kept; one they gave (in the
    /// uploads) is kept in memory only, to be read.
    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        use crate::platform::{DOWNLOADS, UPLOADS};
        if path.starts_with(DOWNLOADS) {
            crate::platform::download(&path.file_name().unwrap_or_default().to_string_lossy(), bytes);
            return Ok(());
        }
        let now = crate::platform::since_epoch().as_secs();
        FILES.with_borrow_mut(|f| f.write(path, bytes, now));
        if !path.starts_with(UPLOADS) {
            files_put(&path.to_string_lossy(), bytes, now as f64);
        }
        Ok(())
    }

    pub fn create_dir_all(_dir: &Path) -> io::Result<()> {
        Ok(())
    }

    pub fn remove_file(path: &Path) -> io::Result<()> {
        FILES.with_borrow_mut(|f| f.remove(path))?;
        files_delete(&path.to_string_lossy());
        Ok(())
    }

    pub fn read_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
        FILES.with_borrow(|f| f.read_dir(dir))
    }

    pub fn exists(path: &Path) -> bool {
        FILES.with_borrow(|f| f.exists(path))
    }

    pub fn modified(path: &Path) -> Option<u64> {
        FILES.with_borrow(|f| f.modified(path))
    }

    /// A page keeps no record of when a file was made: when it was last written will do.
    pub fn made(path: &Path) -> Option<u64> {
        modified(path)
    }

    pub fn canonicalize(path: &Path) -> PathBuf {
        path.to_owned()
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::load;
#[cfg(target_arch = "wasm32")]
use web as sys;

pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    sys::read(path.as_ref())
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    sys::write(path.as_ref(), contents.as_ref())
}

pub fn create_dir_all(dir: impl AsRef<Path>) -> io::Result<()> {
    sys::create_dir_all(dir.as_ref())
}

pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    sys::remove_file(path.as_ref())
}

/// The files and folders in `dir`, as paths.
pub fn read_dir(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    sys::read_dir(dir.as_ref())
}

pub fn exists(path: impl AsRef<Path>) -> bool {
    sys::exists(path.as_ref())
}

/// When the file was last written, in seconds since 1970.
pub fn modified(path: impl AsRef<Path>) -> Option<u64> {
    sys::modified(path.as_ref())
}

/// When the file was made (or, where that is not known, last written), in seconds since 1970.
pub fn made(path: impl AsRef<Path>) -> Option<u64> {
    sys::made(path.as_ref())
}

/// The full path, so a file reached two ways is the same file. As it is, where it cannot be found.
pub fn canonicalize(path: impl AsRef<Path>) -> PathBuf {
    sys::canonicalize(path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::Memory;
    use std::path::{Path, PathBuf};

    #[test]
    fn the_browser_s_files_are_read_back_as_written_and_folders_hold_what_is_in_them() {
        let mut m = Memory::default();
        m.write(Path::new("/caprice/versions/a1/100.caprice"), b"one", 100);
        m.write(Path::new("/caprice/versions/a1/story.json"), b"{}", 100);
        m.write(Path::new("/caprice/versions/b2/200.caprice"), b"two", 200);
        m.write(Path::new("/caprice/theme.json"), b"\"City\"", 300);
        assert_eq!(m.read(Path::new("/caprice/versions/b2/200.caprice")).unwrap(), b"two");
        let folders = m.read_dir(Path::new("/caprice/versions")).unwrap();
        let (a1, b2) = (PathBuf::from("/caprice/versions/a1"), PathBuf::from("/caprice/versions/b2"));
        assert_eq!(folders, [a1, b2], "each story's folder once");
        let top = m.read_dir(Path::new("/caprice")).unwrap();
        assert_eq!(top, [PathBuf::from("/caprice/theme.json"), PathBuf::from("/caprice/versions")]);
        assert!(m.exists(Path::new("/caprice/versions/a1")) && m.exists(Path::new("/caprice/theme.json")));
        assert_eq!(m.modified(Path::new("/caprice/theme.json")), Some(300));
    }

    #[test]
    fn a_folder_of_the_browser_s_files_is_gone_with_its_last_file() {
        let mut m = Memory::default();
        m.write(Path::new("/caprice/versions/a1/100.caprice"), b"one", 100);
        m.remove(Path::new("/caprice/versions/a1/100.caprice")).unwrap();
        assert!(!m.exists(Path::new("/caprice/versions/a1")));
        assert!(m.read_dir(Path::new("/caprice/versions")).is_err(), "as a folder never made");
        assert!(m.remove(Path::new("/caprice/versions/a1/100.caprice")).is_err());
        assert!(m.read(Path::new("/caprice/nothing.json")).is_err());
        // A name that only begins like the folder's is not in it.
        m.write(Path::new("/caprice/versions-old/x"), b"", 1);
        assert!(m.read_dir(Path::new("/caprice/versions")).is_err());
        assert_eq!(m.paths().count(), 1);
    }
}
