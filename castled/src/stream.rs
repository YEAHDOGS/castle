//! Sending a file to a client, whole or in part.
//!
//! A video player asks for the piece of a file it needs next ("Range: bytes=1000-1999")
//! rather than the whole thing, and seeks by asking for another piece. Every file the box
//! serves goes through here so previews, downloads and playback all support that.

use std::{io::SeekFrom, path::Path};

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// How a file is offered to the browser.
#[derive(Clone, Copy, PartialEq)]
pub enum Offer {
    /// As a download: the browser must not show or run it. For vault and computer files.
    Download,
    /// To be shown in place: a film in a player, a photo in a page.
    Inline,
}

/// The first and last byte a `Range` header asks for, for a file of `size` bytes.
/// `Ok(None)` when there is no range to honour (send the whole file); `Err(())` when the
/// range lies outside the file.
fn byte_range(range: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(spec) = range.and_then(|r| r.trim().strip_prefix("bytes=")) else { return Ok(None) };
    // Several ranges at once ("0-9,20-29") are rare and need a multipart answer: send it all.
    if spec.contains(',') {
        return Ok(None);
    }
    let Some((first, last)) = spec.split_once('-') else { return Ok(None) };
    let (first, last) = (first.trim(), last.trim());
    if size == 0 {
        return Err(());
    }
    let (start, end) = match (first.is_empty(), last.is_empty()) {
        // "-500": the last 500 bytes.
        (true, false) => {
            let count: u64 = last.parse().map_err(|_| ())?;
            if count == 0 {
                return Err(());
            }
            (size.saturating_sub(count), size - 1)
        }
        (false, true) => (first.parse().map_err(|_| ())?, size - 1),
        (false, false) => (first.parse().map_err(|_| ())?, last.parse::<u64>().map_err(|_| ())?.min(size - 1)),
        (true, true) => return Ok(None),
    };
    if start > end || start >= size { Err(()) } else { Ok(Some((start, end))) }
}

/// A response that sends `path`, honouring the request's `Range` header if it has one.
pub async fn file(path: &Path, range: Option<&HeaderValue>, mime: &str, offer: Offer) -> std::io::Result<Response> {
    let mut handle = tokio::fs::File::open(path).await?;
    let size = handle.metadata().await?.len();
    let wanted = byte_range(range.and_then(|v| v.to_str().ok()), size);

    let mut res = match wanted {
        Err(()) => {
            let mut res = Response::new(Body::empty());
            *res.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
                res.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            return Ok(res);
        }
        Ok(Some((start, end))) => {
            handle.seek(SeekFrom::Start(start)).await?;
            let length = end - start + 1;
            let mut res = Response::new(Body::from_stream(tokio_util::io::ReaderStream::new(handle.take(length))));
            *res.status_mut() = StatusCode::PARTIAL_CONTENT;
            res.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(length));
            if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")) {
                res.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            res
        }
        Ok(None) => {
            let mut res = Response::new(Body::from_stream(tokio_util::io::ReaderStream::new(handle)));
            res.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(size));
            res
        }
    };

    let headers = res.headers_mut();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(v) = HeaderValue::from_str(mime) {
        headers.insert(header::CONTENT_TYPE, v);
    }
    // Files are data, never pages: the browser must not guess a type and run one.
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if offer == Offer::Download {
        headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_read_the_way_players_send_them() {
        assert_eq!(byte_range(None, 1000), Ok(None));
        assert_eq!(byte_range(Some("bytes=0-99"), 1000), Ok(Some((0, 99))));
        assert_eq!(byte_range(Some("bytes=900-"), 1000), Ok(Some((900, 999))));
        assert_eq!(byte_range(Some("bytes=-100"), 1000), Ok(Some((900, 999))));
        // Past the end is trimmed to the end; a start past the end is refused.
        assert_eq!(byte_range(Some("bytes=990-5000"), 1000), Ok(Some((990, 999))));
        assert_eq!(byte_range(Some("bytes=1000-"), 1000), Err(()));
        assert_eq!(byte_range(Some("bytes=50-10"), 1000), Err(()));
        assert_eq!(byte_range(Some("bytes=0-"), 0), Err(()));
        // Anything else is answered with the whole file.
        assert_eq!(byte_range(Some("bytes=0-9,20-29"), 1000), Ok(None));
        assert_eq!(byte_range(Some("items=0-9"), 1000), Ok(None));
        assert_eq!(byte_range(Some("bytes=abc-"), 1000), Err(()));
    }

    #[tokio::test]
    async fn a_part_of_a_file_is_sent_when_a_part_is_asked_for() {
        let path = std::env::temp_dir().join(format!("castled-stream-{}.bin", std::process::id()));
        std::fs::write(&path, b"0123456789").unwrap();
        let range = HeaderValue::from_static("bytes=2-5");
        let res = file(&path, Some(&range), "application/octet-stream", Offer::Inline).await.unwrap();
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(res.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
        assert!(res.headers().get(header::CONTENT_DISPOSITION).is_none());
        let body = axum::body::to_bytes(res.into_body(), 100).await.unwrap();
        assert_eq!(&body[..], b"2345");

        let whole = file(&path, None, "application/octet-stream", Offer::Download).await.unwrap();
        assert_eq!(whole.status(), StatusCode::OK);
        assert_eq!(whole.headers()[header::CONTENT_DISPOSITION], "attachment");
        assert_eq!(&axum::body::to_bytes(whole.into_body(), 100).await.unwrap()[..], b"0123456789");
        std::fs::remove_file(path).unwrap();
    }
}
