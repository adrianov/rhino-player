// `.ts` is MPEG transport stream or TypeScript — sniff before listing / catalog.

use std::io::Read;
use std::path::Path;

const HEAD: usize = 512;
const TS_PACKET: usize = 188;
const TS_SYNC: u8 = 0x47;

/// True when a `.ts` file is a video transport stream, not a TypeScript source.
pub(super) fn ts_file_is_video(path: &Path) -> bool {
    let Some(head) = read_head(path, HEAD) else {
        return false;
    };
    if mpeg_ts_sync(&head) {
        return true;
    }
    let (mime, _) = gtk::gio::content_type_guess(Some(path), &head);
    let mime = mime.as_str().to_ascii_lowercase();
    if mime_is_source(&mime) {
        return false;
    }
    mime_is_video(&mime) && !looks_like_text(&head)
}

fn read_head(path: &Path, n: usize) -> Option<Vec<u8>> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0_u8; n];
    let got = f.read(&mut buf).ok()?;
    buf.truncate(got);
    Some(buf)
}

fn mpeg_ts_sync(head: &[u8]) -> bool {
    head.len() > TS_PACKET && head[0] == TS_SYNC && head[TS_PACKET] == TS_SYNC
}

fn mime_is_video(mime: &str) -> bool {
    mime.starts_with("video/")
}

fn mime_is_source(mime: &str) -> bool {
    mime.starts_with("text/") || mime.contains("typescript") || mime.contains("javascript")
}

fn looks_like_text(head: &[u8]) -> bool {
    let n = head.len().min(64);
    if n == 0 {
        return false;
    }
    head[..n]
        .iter()
        .filter(|&&b| b == 9 || b == 10 || b == 13 || (32..=126).contains(&b))
        .count()
        * 4
        >= n * 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "rhino-ts-mime-{name}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn typescript_source_is_not_video() {
        let dir = scratch("src");
        let p = dir.join("app.ts");
        fs::write(&p, "export const x = 1;\n").unwrap();
        assert!(!ts_file_is_video(&p));
        assert!(!crate::video_ext::is_video_path(&p));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mpeg_ts_sync_is_video() {
        let dir = scratch("mpts");
        let p = dir.join("clip.ts");
        let mut bytes = vec![0_u8; TS_PACKET + 1];
        bytes[0] = TS_SYNC;
        bytes[TS_PACKET] = TS_SYNC;
        fs::write(&p, bytes).unwrap();
        assert!(ts_file_is_video(&p));
        assert!(crate::video_ext::is_video_path(&p));
        let _ = fs::remove_dir_all(&dir);
    }
}
