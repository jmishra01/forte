//! `notefile://` serves images from the open vault to the preview, and nothing
//! else: the path is resolved inside whichever vault is open at the time of each
//! request, so after switching vaults the old one can't be read any more.

use std::fs;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext};

use crate::vault::{percent_decode, resolve, root, AppState};

pub const SCHEME: &str = "notefile";

fn mime(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let rel = percent_decode(request.uri().path().trim_start_matches('/'));
    let not_found = || Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap();
    let Some(kind) = mime(&rel) else { return not_found() };
    let state = ctx.app_handle().state::<AppState>();
    let Ok(path) = resolve(&root(&state), &rel) else { return not_found() };
    match fs::read(path) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, kind)
            // SVGs could carry scripts; images never need to run any.
            .header("Content-Security-Policy", "default-src 'none'; style-src 'unsafe-inline'")
            .body(bytes)
            .unwrap(),
        Err(_) => not_found(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_images_are_served() {
        assert_eq!(mime("attachments/a.PNG"), Some("image/png"));
        assert_eq!(mime("a.svg"), Some("image/svg+xml"));
        assert_eq!(mime("notes/secret.md"), None);
        assert_eq!(mime("no-extension"), None);
    }
}
