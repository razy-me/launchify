use std::fs;
use std::path::PathBuf;

use crate::storage::write_atomic;

/// One-time on-disk PNG icon cache.
#[derive(Debug, Clone)]
pub struct IconCache {
    cache_dir: PathBuf,
}

impl IconCache {
    /// Creates a new `IconCache` rooted at `cache_dir`.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Returns the sanitized PNG path for `key` inside `cache_dir`.
    pub fn icon_path(&self, key: &str) -> PathBuf {
        let sanitized: String = key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let name = if sanitized.is_empty() {
            "icon".to_string()
        } else {
            sanitized
        };
        self.cache_dir.join(format!("{name}.png"))
    }

    /// Returns `true` if a cached PNG already exists for `key`.
    pub fn has_cached(&self, key: &str) -> bool {
        self.icon_path(key).is_file()
    }

    /// Writes raw PNG bytes atomically for `key` and returns the cached path.
    pub fn write_cached_png(&self, key: &str, png_bytes: &[u8]) -> Result<PathBuf, String> {
        fs::create_dir_all(&self.cache_dir)
            .map_err(|e| format!("failed to create icon cache dir: {e}"))?;
        let path = self.icon_path(key);
        write_atomic(&path, png_bytes)?;
        Ok(path)
    }

    /// Returns the cached PNG path for `key` if it exists, or extracts and caches it from `target`.
    pub fn get_or_extract(&self, key: &str, target: &str) -> Option<PathBuf> {
        let path = self.icon_path(key);
        if path.is_file() {
            return Some(path);
        }

        let png_bytes = extract_png_for_target(target)?;
        self.write_cached_png(key, &png_bytes).ok()
    }
}

#[cfg(windows)]
fn extract_png_for_target(target: &str) -> Option<Vec<u8>> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject,
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let wide: Vec<u16> = OsStr::new(target)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut shfi = SHFILEINFOW::default();
        let res = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if res == 0 || shfi.hIcon.is_invalid() {
            return None;
        }
        let hicon = shfi.hIcon;

        let mut icon_info = ICONINFO::default();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            let _ = DestroyIcon(hicon);
            return None;
        }

        let mut bmp = BITMAP::default();
        let got_obj = if !icon_info.hbmColor.is_invalid() {
            GetObjectW(
                icon_info.hbmColor,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut _),
            ) != 0
        } else {
            false
        };

        let mut rgba_out: Option<Vec<u8>> = None;
        if got_obj && bmp.bmWidth > 0 && bmp.bmHeight > 0 {
            let w = bmp.bmWidth as u32;
            let h = bmp.bmHeight as u32;
            let hdc = CreateCompatibleDC(None);
            if !hdc.is_invalid() {
                let old = SelectObject(hdc, icon_info.hbmColor);
                let mut bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w as i32,
                        biHeight: -(h as i32), // top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bgra = vec![0u8; (w * h * 4) as usize];
                let lines = GetDIBits(
                    hdc,
                    icon_info.hbmColor,
                    0,
                    h,
                    Some(bgra.as_mut_ptr() as *mut _),
                    &mut bi,
                    DIB_RGB_COLORS,
                );
                SelectObject(hdc, old);
                let _ = DeleteDC(hdc);

                if lines > 0 {
                    for px in bgra.chunks_exact_mut(4) {
                        px.swap(0, 2); // BGRA -> RGBA
                    }
                    rgba_out = Some(encode_rgba_png(w, h, &bgra));
                }
            }
        }

        if !icon_info.hbmColor.is_invalid() {
            let _ = DeleteObject(icon_info.hbmColor);
        }
        if !icon_info.hbmMask.is_invalid() {
            let _ = DeleteObject(icon_info.hbmMask);
        }
        let _ = DestroyIcon(hicon);

        rgba_out
    }
}

#[cfg(not(windows))]
fn extract_png_for_target(_target: &str) -> Option<Vec<u8>> {
    None
}

/// Minimal dependency-free RGBA -> PNG encoder using uncompressed deflate blocks.
#[cfg(any(windows, test))]
fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr);

    let row_bytes = (width as usize) * 4;
    let mut raw = Vec::with_capacity((row_bytes + 1) * (height as usize));
    for y in 0..(height as usize) {
        raw.push(0); // filter type 0
        let start = y * row_bytes;
        raw.extend_from_slice(&rgba[start..start + row_bytes]);
    }

    let mut zlib = Vec::with_capacity(raw.len() + 16);
    zlib.push(0x78);
    zlib.push(0x01);
    let mut offset = 0;
    while offset < raw.len() {
        let chunk_len = (raw.len() - offset).min(65535);
        let is_last = offset + chunk_len == raw.len();
        zlib.push(if is_last { 1 } else { 0 });
        let len_u16 = chunk_len as u16;
        zlib.extend_from_slice(&len_u16.to_le_bytes());
        zlib.extend_from_slice(&(!len_u16).to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + chunk_len]);
        offset += chunk_len;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    write_png_chunk(&mut out, b"IDAT", &zlib);
    write_png_chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(any(windows, test))]
fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_buf = Vec::with_capacity(4 + data.len());
    crc_buf.extend_from_slice(kind);
    crc_buf.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_buf).to_be_bytes());
}

#[cfg(any(windows, test))]
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(any(windows, test))]
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_cache_writes_and_reuses_without_reextracting() {
        let dir = std::env::temp_dir().join(format!("launchify_icon_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = IconCache::new(dir.clone());

        assert!(!cache.has_cached("app:code"));
        let png = encode_rgba_png(1, 1, &[255, 0, 0, 255]);
        let written = cache.write_cached_png("app:code", &png).expect("write png");
        assert!(cache.has_cached("app:code"));

        // Overwrite with sentinel bytes? No, get_or_extract should return existing path directly.
        let reused = cache
            .get_or_extract("app:code", "nonexistent_target_that_would_fail_extraction")
            .expect("should reuse cached png");
        assert_eq!(written, reused);
        assert_eq!(fs::read(&reused).unwrap(), png);

        let _ = fs::remove_dir_all(&dir);
    }
}
