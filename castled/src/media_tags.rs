//! What a music file says about itself: title, artist, album, track number, year, genre
//! and the cover picture inside it. Reads ID3v2 (MP3 and others) and FLAC's own tags.
//! Only the start of the file is read, never more than MAX_TAG_BYTES.

use std::{io::Read, path::Path};

/// Tags live at the start of a file; a cover picture can make them a few megabytes.
const MAX_TAG_BYTES: usize = 16 * 1024 * 1024;
const MAX_PICTURE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track: u32,
    pub year: u32,
    pub genre: String,
    /// The embedded cover: its type ("image/jpeg") and bytes.
    pub picture: Option<(String, Vec<u8>)>,
}

/// Reads what it can. A file with no tags, or tags this does not know, gives empty Tags.
pub fn read(path: &Path) -> Tags {
    let Ok(mut file) = std::fs::File::open(path) else { return Tags::default() };
    let mut head = [0u8; 10];
    if file.read_exact(&mut head).is_err() {
        return Tags::default();
    }
    if &head[..3] == b"ID3" {
        let size = synchsafe(&head[6..10]) as usize;
        if size == 0 || size > MAX_TAG_BYTES {
            return Tags::default();
        }
        let mut body = vec![0u8; size];
        if file.read_exact(&mut body).is_err() {
            return Tags::default();
        }
        return id3(head[3], head[5], &body);
    }
    if &head[..4] == b"fLaC" {
        let mut rest = Vec::new();
        let _ = file.take(MAX_TAG_BYTES as u64).read_to_end(&mut rest);
        let mut all = head[4..].to_vec();
        all.extend(rest);
        return flac(&all);
    }
    Tags::default()
}

fn synchsafe(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |acc, &x| (acc << 7) | u32::from(x & 0x7f))
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// "3/12" -> 3; "2019-05-01" -> 2019.
fn leading_number(text: &str) -> u32 {
    text.trim().chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)
}

/// Text in one of ID3's four encodings.
fn id3_text(encoding: u8, bytes: &[u8]) -> String {
    let text = match encoding {
        1 | 2 => {
            let (big, body) = match bytes {
                [0xFE, 0xFF, rest @ ..] => (true, rest),
                [0xFF, 0xFE, rest @ ..] => (false, rest),
                _ => (encoding == 2, bytes),
            };
            let units: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| if big { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
                .collect();
            String::from_utf16_lossy(&units)
        }
        3 => String::from_utf8_lossy(bytes).to_string(),
        _ => bytes.iter().map(|&b| b as char).collect(),
    };
    text.trim_matches(char::from(0)).trim().to_string()
}

/// "(17)Rock" or "(17)" -> the name; ID3v1 genres by number are mostly shown as written.
fn genre_name(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.strip_prefix('(').and_then(|r| r.split_once(')')) {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => trimmed.to_string(),
    }
}

fn id3(major: u8, flags: u8, body: &[u8]) -> Tags {
    let mut tags = Tags::default();
    // An extended header, when flagged, comes first; skip it.
    let mut at = 0usize;
    if flags & 0x40 != 0 && body.len() >= 4 {
        let len = if major >= 4 { synchsafe(&body[..4]) as usize } else { be32(&body[..4]) as usize + 4 };
        at = len.min(body.len());
    }
    // ID3v2.2 has three-letter frames and is rare enough to leave out.
    if major < 3 {
        return tags;
    }
    while at + 10 <= body.len() {
        let id = &body[at..at + 4];
        if id[0] == 0 {
            break; // padding
        }
        let size = if major >= 4 { synchsafe(&body[at + 4..at + 8]) } else { be32(&body[at + 4..at + 8]) } as usize;
        let start = at + 10;
        let end = start.saturating_add(size);
        if size == 0 || end > body.len() {
            break;
        }
        let frame = &body[start..end];
        at = end;
        let text = || id3_text(frame[0], &frame[1..]);
        match id {
            b"TIT2" => tags.title = text(),
            b"TPE1" => tags.artist = text(),
            b"TPE2" if tags.artist.is_empty() => tags.artist = text(),
            b"TALB" => tags.album = text(),
            b"TRCK" => tags.track = leading_number(&text()),
            b"TYER" | b"TDRC" => tags.year = leading_number(&text()),
            b"TCON" => tags.genre = genre_name(&text()),
            b"APIC" if tags.picture.is_none() && frame.len() > 4 => tags.picture = apic(frame),
            _ => {}
        }
    }
    tags
}

/// An ID3 picture frame: encoding, type\0, picture kind, description\0, data.
fn apic(frame: &[u8]) -> Option<(String, Vec<u8>)> {
    let encoding = frame[0];
    let rest = &frame[1..];
    let mime_end = rest.iter().position(|&b| b == 0)?;
    let mime = String::from_utf8_lossy(&rest[..mime_end]).to_string();
    let rest = rest.get(mime_end + 2..)?; // the \0 and the picture kind byte
    // The description ends with one zero byte, or two in the UTF-16 encodings.
    let data_start = if encoding == 1 || encoding == 2 {
        rest.chunks_exact(2).position(|c| c == [0, 0]).map(|i| i * 2 + 2)?
    } else {
        rest.iter().position(|&b| b == 0)? + 1
    };
    let data = rest.get(data_start..)?;
    if data.is_empty() || data.len() > MAX_PICTURE_BYTES {
        return None;
    }
    let mime = if mime.contains('/') { mime } else { format!("image/{}", mime.to_lowercase()).replace("jpg", "jpeg") };
    Some((mime, data.to_vec()))
}

/// FLAC: metadata blocks after "fLaC". Block 4 holds "KEY=value" comments, block 6 a picture.
fn flac(bytes: &[u8]) -> Tags {
    let mut tags = Tags::default();
    let mut at = 0usize;
    while at + 4 <= bytes.len() {
        let last = bytes[at] & 0x80 != 0;
        let kind = bytes[at] & 0x7f;
        let len = (usize::from(bytes[at + 1]) << 16) | (usize::from(bytes[at + 2]) << 8) | usize::from(bytes[at + 3]);
        let start = at + 4;
        let Some(block) = bytes.get(start..start + len) else { break };
        match kind {
            4 => vorbis_comments(block, &mut tags),
            6 if tags.picture.is_none() => tags.picture = flac_picture(block),
            _ => {}
        }
        at = start + len;
        if last {
            break;
        }
    }
    tags
}

fn le32(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn vorbis_comments(block: &[u8], tags: &mut Tags) {
    let Some(vendor) = le32(block, 0) else { return };
    let mut at = 4 + vendor;
    let Some(count) = le32(block, at) else { return };
    at += 4;
    for _ in 0..count.min(1000) {
        let Some(len) = le32(block, at) else { return };
        let Some(raw) = block.get(at + 4..at + 4 + len) else { return };
        at += 4 + len;
        let text = String::from_utf8_lossy(raw);
        let Some((key, value)) = text.split_once('=') else { continue };
        let value = value.trim().to_string();
        match key.to_ascii_uppercase().as_str() {
            "TITLE" => tags.title = value,
            "ARTIST" => tags.artist = value,
            "ALBUMARTIST" if tags.artist.is_empty() => tags.artist = value,
            "ALBUM" => tags.album = value,
            "TRACKNUMBER" => tags.track = leading_number(&value),
            "DATE" | "YEAR" => tags.year = leading_number(&value),
            "GENRE" => tags.genre = value,
            _ => {}
        }
    }
}

fn flac_picture(block: &[u8]) -> Option<(String, Vec<u8>)> {
    let be = |at: usize| -> Option<usize> { Some(u32::from_be_bytes(block.get(at..at + 4)?.try_into().ok()?) as usize) };
    let mime_len = be(4)?;
    let mime = String::from_utf8_lossy(block.get(8..8 + mime_len)?).to_string();
    let desc_at = 8 + mime_len;
    let desc_len = be(desc_at)?;
    let data_len_at = desc_at + 4 + desc_len + 16; // width, height, depth, colours
    let data_len = be(data_len_at)?;
    if data_len > MAX_PICTURE_BYTES {
        return None;
    }
    let data = block.get(data_len_at + 4..data_len_at + 4 + data_len)?;
    Some((mime, data.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend((body.len() as u32).to_be_bytes());
        out.extend([0, 0]);
        out.extend(body);
        out
    }

    fn text_frame(id: &[u8; 4], text: &str) -> Vec<u8> {
        let mut body = vec![3u8];
        body.extend(text.as_bytes());
        frame(id, &body)
    }

    #[test]
    fn id3_tags_and_cover_are_read() {
        let mut frames = Vec::new();
        frames.extend(text_frame(b"TIT2", "Midnight City"));
        frames.extend(text_frame(b"TPE1", "M83"));
        frames.extend(text_frame(b"TALB", "Hurry Up, We're Dreaming"));
        frames.extend(text_frame(b"TRCK", "2/22"));
        frames.extend(text_frame(b"TYER", "2011"));
        frames.extend(text_frame(b"TCON", "(52)Electronic"));
        let mut apic = vec![0u8];
        apic.extend(b"image/png\0");
        apic.push(3);
        apic.extend(b"cover\0");
        apic.extend([0x89, b'P', b'N', b'G']);
        frames.extend(frame(b"APIC", &apic));
        frames.extend([0u8; 16]); // padding

        let mut file = b"ID3\x03\x00\x00".to_vec();
        let size = frames.len() as u32;
        file.extend([(size >> 21) as u8 & 0x7f, (size >> 14) as u8 & 0x7f, (size >> 7) as u8 & 0x7f, size as u8 & 0x7f]);
        file.extend(&frames);
        file.extend([0xFF, 0xFB, 0x90, 0x00]); // the audio would start here

        let path = std::env::temp_dir().join(format!("castled-tags-{}.mp3", crate::random_hex(6)));
        std::fs::write(&path, &file).unwrap();
        let tags = read(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(tags.title, "Midnight City");
        assert_eq!(tags.artist, "M83");
        assert_eq!(tags.album, "Hurry Up, We're Dreaming");
        assert_eq!((tags.track, tags.year), (2, 2011));
        assert_eq!(tags.genre, "Electronic");
        assert_eq!(tags.picture, Some(("image/png".to_string(), vec![0x89, b'P', b'N', b'G'])));
    }

    #[test]
    fn utf16_text_is_read() {
        assert_eq!(id3_text(1, &[0xFF, 0xFE, b'h', 0, b'i', 0, 0, 0]), "hi");
        assert_eq!(id3_text(2, &[0, b'o', 0, b'k']), "ok");
    }

    #[test]
    fn flac_comments_are_read() {
        let mut comments = Vec::new();
        comments.extend(3u32.to_le_bytes());
        comments.extend(b"enc");
        let entries = ["TITLE=So What", "ARTIST=Miles Davis", "ALBUM=Kind of Blue", "TRACKNUMBER=1", "DATE=1959-08-17"];
        comments.extend((entries.len() as u32).to_le_bytes());
        for entry in entries {
            comments.extend((entry.len() as u32).to_le_bytes());
            comments.extend(entry.as_bytes());
        }
        let mut file = b"fLaC".to_vec();
        file.push(0x80 | 4);
        let len = comments.len();
        file.extend([(len >> 16) as u8, (len >> 8) as u8, len as u8]);
        file.extend(&comments);
        let path = std::env::temp_dir().join(format!("castled-tags-{}.flac", crate::random_hex(6)));
        std::fs::write(&path, &file).unwrap();
        let tags = read(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!((tags.title.as_str(), tags.artist.as_str(), tags.album.as_str()), ("So What", "Miles Davis", "Kind of Blue"));
        assert_eq!((tags.track, tags.year), (1, 1959));
    }

    #[test]
    fn a_file_without_tags_gives_nothing() {
        let path = std::env::temp_dir().join(format!("castled-tags-{}.mp3", crate::random_hex(6)));
        std::fs::write(&path, b"not a tagged file").unwrap();
        assert_eq!(read(&path), Tags::default());
        std::fs::remove_file(&path).unwrap();
    }
}
