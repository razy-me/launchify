use base64::Engine as _;
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub output_path: String,
    pub bytes_written: u64,
}

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "gif", "ico", "tiff", "tga", "qoi", "pnm",
];
const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "flv", "wmv", "m4v"];
const AUDIO_EXTS: &[&str] = &["mp3", "wav", "flac", "ogg", "aac", "m4a", "opus", "wma"];
const DATA_EXTS: &[&str] = &["json", "yaml", "toml", "csv", "tsv"];
const PANDOC_EXTS: &[&str] = &["docx", "odt", "rtf", "epub", "rst", "tex"];

fn normalize_ext(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_ascii_lowercase()
}

/// Returns supported target extensions for a given source file extension.
pub fn supported_targets(ext: &str) -> Vec<&'static str> {
    let norm = normalize_ext(ext);
    if norm.is_empty() {
        return Vec::new();
    }

    if IMAGE_EXTS.contains(&norm.as_str()) {
        let mut targets: Vec<&'static str> = IMAGE_EXTS
            .iter()
            .copied()
            .filter(|&f| {
                if norm == "jpg" || norm == "jpeg" {
                    f != "jpg" && f != "jpeg"
                } else {
                    f != norm
                }
            })
            .collect();
        targets.push("pdf");
        return targets;
    }

    if DATA_EXTS.contains(&norm.as_str()) || norm == "yml" {
        return DATA_EXTS
            .iter()
            .copied()
            .filter(|&f| {
                if norm == "yml" {
                    f != "yaml"
                } else {
                    f != norm
                }
            })
            .collect();
    }

    match norm.as_str() {
        "md" | "markdown" => return vec!["html", "txt", "pdf"],
        "html" | "htm" => return vec!["txt", "pdf"],
        "txt" => return vec!["base64", "hex", "url"],
        "base64" | "b64" => return vec!["txt"],
        "hex" => return vec!["txt"],
        "url" => return vec!["txt"],
        "pdf" => return vec!["txt", "png", "jpg"],
        _ => {}
    }

    if VIDEO_EXTS.contains(&norm.as_str()) {
        let mut targets: Vec<&'static str> = VIDEO_EXTS
            .iter()
            .copied()
            .filter(|&f| f != norm)
            .collect();
        targets.push("gif");
        targets.extend_from_slice(AUDIO_EXTS);
        return targets;
    }

    if AUDIO_EXTS.contains(&norm.as_str()) {
        return AUDIO_EXTS
            .iter()
            .copied()
            .filter(|&f| f != norm)
            .collect();
    }

    if PANDOC_EXTS.contains(&norm.as_str()) {
        return vec!["md", "html", "txt", "pdf"];
    }

    Vec::new()
}

fn build_output_path(source: &Path, target_ext: &str) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let primary = parent.join(format!("{stem}.{target_ext}"));
    if !primary.exists() {
        return primary;
    }
    let converted = parent.join(format!("{stem}_converted.{target_ext}"));
    if !converted.exists() {
        return converted;
    }
    let mut counter = 2u32;
    loop {
        let candidate = parent.join(format!("{stem}_converted_{counter}.{target_ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn image_format_from_ext(ext: &str) -> Option<ImageFormat> {
    match ext {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        "gif" => Some(ImageFormat::Gif),
        "ico" => Some(ImageFormat::Ico),
        "tiff" => Some(ImageFormat::Tiff),
        "tga" => Some(ImageFormat::Tga),
        "qoi" => Some(ImageFormat::Qoi),
        "pnm" | "ppm" | "pgm" | "pbm" => Some(ImageFormat::Pnm),
        _ => None,
    }
}

fn convert_image_to_pdf(source: &Path, out_path: &Path) -> Result<(), String> {
    let img = image::open(source)
        .map_err(|e| format!("Failed to open image {}: {e}", source.display()))?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let raw = rgb.into_raw();

    let mut pdf = Vec::with_capacity(raw.len() + 1024);
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let mut offsets = [0usize; 6];

    offsets[1] = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    offsets[2] = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    offsets[3] = pdf.len();
    pdf.extend_from_slice(
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>\nendobj\n"
        )
        .as_bytes(),
    );

    offsets[4] = pdf.len();
    pdf.extend_from_slice(
        format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",
            raw.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&raw);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let content_stream = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
    offsets[5] = pdf.len();
    pdf.extend_from_slice(
        format!(
            "5 0 obj\n<< /Length {} >>\nstream\n{content_stream}\nendstream\nendobj\n",
            content_stream.len()
        )
        .as_bytes(),
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for &off in &offsets[1..] {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes(),
    );

    fs::write(out_path, pdf).map_err(|e| format!("Failed to write PDF {}: {e}", out_path.display()))
}

fn convert_image(source: &Path, out_path: &Path, target_ext: &str) -> Result<(), String> {
    if target_ext == "pdf" {
        return convert_image_to_pdf(source, out_path);
    }
    let fmt = image_format_from_ext(target_ext)
        .ok_or_else(|| format!("Unsupported image target format: {target_ext}"))?;
    let img = image::open(source)
        .map_err(|e| format!("Failed to open image {}: {e}", source.display()))?;

    let to_save = if fmt == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(img.to_rgb8())
    } else {
        img
    };

    to_save
        .save_with_format(out_path, fmt)
        .map_err(|e| format!("Failed to save image {}: {e}", out_path.display()))
}

fn parse_data_to_json(source: &Path, src_ext: &str) -> Result<serde_json::Value, String> {
    let content = fs::read_to_string(source)
        .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
    match src_ext {
        "json" => serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}")),
        "yaml" | "yml" => serde_yaml::from_str(&content).map_err(|e| format!("Invalid YAML: {e}")),
        "toml" => {
            let val: toml::Value =
                toml::from_str(&content).map_err(|e| format!("Invalid TOML: {e}"))?;
            serde_json::to_value(val).map_err(|e| format!("TOML conversion error: {e}"))
        }
        "csv" | "tsv" => {
            let delimiter = if src_ext == "tsv" { b'\t' } else { b',' };
            let label = src_ext.to_ascii_uppercase();
            let mut reader = csv::ReaderBuilder::new()
                .delimiter(delimiter)
                .from_reader(content.as_bytes());
            let headers = reader
                .headers()
                .map_err(|e| format!("Invalid {label} headers: {e}"))?
                .clone();
            let mut rows = Vec::new();
            for result in reader.records() {
                let record = result.map_err(|e| format!("Invalid {label} row: {e}"))?;
                let mut obj = serde_json::Map::new();
                for (i, header) in headers.iter().enumerate() {
                    let val = record.get(i).unwrap_or("");
                    obj.insert(
                        header.to_string(),
                        serde_json::Value::String(val.to_string()),
                    );
                }
                rows.push(serde_json::Value::Object(obj));
            }
            Ok(serde_json::Value::Array(rows))
        }
        _ => Err(format!("Unsupported data source format: {src_ext}")),
    }
}

fn json_scalar_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn write_json_as_delimited(
    val: &serde_json::Value,
    out_path: &Path,
    delimiter: u8,
    label: &str,
) -> Result<(), String> {
    let rows = val
        .as_array()
        .ok_or_else(|| format!("{label} export requires a JSON/YAML/TOML array of flat objects"))?;
    if rows.is_empty() {
        return Err(format!("Cannot export empty array to {label}"));
    }

    let mut header_set = BTreeSet::new();
    let mut headers = Vec::new();
    for row in rows {
        let obj = row.as_object().ok_or_else(|| {
            format!("{label} export requires every array element to be an object")
        })?;
        for key in obj.keys() {
            if header_set.insert(key.clone()) {
                headers.push(key.clone());
            }
        }
    }

    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_path(out_path)
        .map_err(|e| format!("Failed to create {label} {}: {e}", out_path.display()))?;
    writer
        .write_record(&headers)
        .map_err(|e| format!("Failed to write {label} headers: {e}"))?;

    for row in rows {
        let obj = row.as_object().expect("validated above");
        let record: Vec<String> = headers
            .iter()
            .map(|h| obj.get(h).map(json_scalar_to_string).unwrap_or_default())
            .collect();
        writer
            .write_record(&record)
            .map_err(|e| format!("Failed to write {label} row: {e}"))?;
    }

    writer
        .flush()
        .map_err(|e| format!("Failed to flush {label} {}: {e}", out_path.display()))
}

fn convert_data(
    source: &Path,
    out_path: &Path,
    src_ext: &str,
    target_ext: &str,
) -> Result<(), String> {
    let val = parse_data_to_json(source, src_ext)?;
    let serialized = match target_ext {
        "json" => serde_json::to_string_pretty(&val)
            .map_err(|e| format!("JSON serialization error: {e}"))?,
        "yaml" | "yml" => {
            serde_yaml::to_string(&val).map_err(|e| format!("YAML serialization error: {e}"))?
        }
        "toml" => {
            toml::to_string_pretty(&val).map_err(|e| format!("TOML serialization error: {e}"))?
        }
        "csv" => return write_json_as_delimited(&val, out_path, b',', "CSV"),
        "tsv" => return write_json_as_delimited(&val, out_path, b'\t', "TSV"),
        _ => return Err(format!("Unsupported data target format: {target_ext}")),
    };
    fs::write(out_path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if !cleaned.is_ascii() {
        return Err("Invalid hex input: contains non-ASCII characters".to_string());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("Invalid hex input: odd number of hex digits".to_string());
    }
    let mut bytes = Vec::with_capacity(cleaned.len() / 2);
    for i in (0..cleaned.len()).step_by(2) {
        let byte = u8::from_str_radix(&cleaned[i..i + 2], 16)
            .map_err(|e| format!("Invalid hex byte '{}': {e}", &cleaned[i..i + 2]))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut skip_block: Option<&str> = None;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let lower_bytes = lower.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if let Some(end_tag) = skip_block {
            if lower_bytes[i..].starts_with(end_tag.as_bytes()) {
                i += end_tag.len();
                skip_block = None;
            } else {
                i += 1;
            }
            continue;
        }

        if bytes[i] == b'<' {
            if lower_bytes[i..].starts_with(b"<script") {
                skip_block = Some("</script>");
                i += 7;
                continue;
            }
            if lower_bytes[i..].starts_with(b"<style") {
                skip_block = Some("</style>");
                i += 6;
                continue;
            }
            in_tag = true;
            if lower_bytes[i..].starts_with(b"<br")
                || lower_bytes[i..].starts_with(b"</p")
                || lower_bytes[i..].starts_with(b"</div")
                || lower_bytes[i..].starts_with(b"</li")
                || lower_bytes[i..].starts_with(b"</h")
            {
                out.push('\n');
            }
            i += 1;
            continue;
        }

        if in_tag {
            if bytes[i] == b'>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }

        let ch = html[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    decoded
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_via_pandoc(source: &Path, out_path: &Path) -> Result<(), String> {
    let output = Command::new("pandoc")
        .arg(source)
        .arg("-o")
        .arg(out_path)
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "pandoc was not found on PATH. Install pandoc to convert document files."
                    .to_string()
            } else {
                format!("Failed to run pandoc: {e}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pandoc conversion failed: {}", stderr.trim()));
    }
    Ok(())
}

fn convert_text(
    source: &Path,
    out_path: &Path,
    src_ext: &str,
    target_ext: &str,
) -> Result<(), String> {
    match (src_ext, target_ext) {
        ("md" | "markdown", "html") => {
            let md = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let parser = pulldown_cmark::Parser::new(&md);
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, parser);
            fs::write(out_path, html)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("md" | "markdown", "txt") => {
            let md = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let parser = pulldown_cmark::Parser::new(&md);
            let mut txt = String::new();
            for event in parser {
                match event {
                    pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                        txt.push_str(&t);
                    }
                    pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                        txt.push('\n');
                    }
                    pulldown_cmark::Event::End(
                        pulldown_cmark::TagEnd::Paragraph
                        | pulldown_cmark::TagEnd::Heading(_)
                        | pulldown_cmark::TagEnd::Item,
                    ) => {
                        txt.push('\n');
                    }
                    _ => {}
                }
            }
            fs::write(out_path, txt.trim_end())
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("md" | "markdown" | "html" | "htm", "pdf") => convert_via_pandoc(source, out_path),
        ("html" | "htm", "txt") => {
            let html = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let txt = strip_html_to_text(&html);
            fs::write(out_path, txt)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("txt", "base64") => {
            let bytes =
                fs::read(source).map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            fs::write(out_path, encoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("base64" | "b64", "txt") => {
            let content = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(content.trim())
                .map_err(|e| format!("Invalid base64 input: {e}"))?;
            fs::write(out_path, decoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("txt", "hex") => {
            let bytes =
                fs::read(source).map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let encoded: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            fs::write(out_path, encoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("hex", "txt") => {
            let content = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let decoded = decode_hex(&content)?;
            fs::write(out_path, decoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("txt", "url") => {
            let content = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let encoded = percent_encoding::utf8_percent_encode(
                &content,
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string();
            fs::write(out_path, encoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        ("url", "txt") => {
            let content = fs::read_to_string(source)
                .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
            let decoded = percent_encoding::percent_decode_str(content.trim())
                .decode_utf8()
                .map_err(|e| format!("Invalid URL-encoded UTF-8: {e}"))?
                .into_owned();
            fs::write(out_path, decoded)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
        }
        _ => Err(format!(
            "Unsupported text conversion: {src_ext} -> {target_ext}"
        )),
    }
}

fn convert_pdf(source: &Path, out_path: &Path, target_ext: &str) -> Result<(), String> {
    match target_ext {
        "txt" => match pdf_extract::extract_text(source) {
            Ok(text) => fs::write(out_path, text)
                .map_err(|e| format!("Failed to write {}: {e}", out_path.display())),
            Err(extract_err) => {
                let output = Command::new("pdftotext")
                    .arg(source)
                    .arg(out_path)
                    .output()
                    .map_err(|e| {
                        if e.kind() == ErrorKind::NotFound {
                            format!(
                                "PDF text extraction failed ({extract_err}) and pdftotext was not found on PATH."
                            )
                        } else {
                            format!("PDF text extraction failed ({extract_err}); pdftotext error: {e}")
                        }
                    })?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("pdftotext failed: {}", stderr.trim()));
                }
                Ok(())
            }
        },
        "png" | "jpg" => {
            let flag = if target_ext == "png" { "-png" } else { "-jpeg" };
            let stem_path = out_path.with_extension("");
            match Command::new("pdftoppm")
                .args([flag, "-singlefile"])
                .arg(source)
                .arg(&stem_path)
                .output()
            {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("pdftoppm failed: {}", stderr.trim()))
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    let mut source_page0 = source.as_os_str().to_os_string();
                    source_page0.push("[0]");
                    let magick_out = Command::new("magick")
                        .arg(&source_page0)
                        .arg(out_path)
                        .output()
                        .map_err(|me| {
                            if me.kind() == ErrorKind::NotFound {
                                "Neither pdftoppm nor magick was found on PATH. Install Poppler or ImageMagick to render PDFs.".to_string()
                            } else {
                                format!("Failed to run magick: {me}")
                            }
                        })?;
                    if !magick_out.status.success() {
                        let stderr = String::from_utf8_lossy(&magick_out.stderr);
                        return Err(format!("magick failed: {}", stderr.trim()));
                    }
                    Ok(())
                }
                Err(e) => Err(format!("Failed to run pdftoppm: {e}")),
            }
        }
        _ => Err(format!("Unsupported PDF target format: {target_ext}")),
    }
}

fn convert_media(source: &Path, out_path: &Path, strip_video: bool) -> Result<(), String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"]).arg(source);
    if strip_video {
        cmd.arg("-vn");
    }
    let output = cmd.arg(out_path).output().map_err(|e| {
        if e.kind() == ErrorKind::NotFound {
            "ffmpeg was not found on PATH. Install ffmpeg to convert audio/video files."
                .to_string()
        } else {
            format!("Failed to run ffmpeg: {e}")
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg conversion failed: {}", stderr.trim()));
    }
    Ok(())
}

/// Converts `source` to `target_ext` in the same directory, avoiding overwriting existing files.
pub fn convert_file(source: &Path, target_ext: &str) -> Result<ConvertResult, String> {
    if !source.is_file() {
        return Err(format!("Source file does not exist: {}", source.display()));
    }

    let src_ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(normalize_ext)
        .filter(|e| !e.is_empty())
        .ok_or_else(|| format!("Source file has no extension: {}", source.display()))?;

    let target = normalize_ext(target_ext);
    if target.is_empty() {
        return Err("Target extension cannot be empty".to_string());
    }

    let allowed = supported_targets(&src_ext);
    if !allowed.contains(&target.as_str()) {
        return Err(format!(
            "Conversion from .{src_ext} to .{target} is not supported"
        ));
    }

    let out_path = build_output_path(source, &target);

    if IMAGE_EXTS.contains(&src_ext.as_str()) {
        convert_image(source, &out_path, &target)?;
    } else if DATA_EXTS.contains(&src_ext.as_str()) || src_ext == "yml" {
        convert_data(source, &out_path, &src_ext, &target)?;
    } else if VIDEO_EXTS.contains(&src_ext.as_str()) {
        let strip_video = AUDIO_EXTS.contains(&target.as_str());
        convert_media(source, &out_path, strip_video)?;
    } else if AUDIO_EXTS.contains(&src_ext.as_str()) {
        convert_media(source, &out_path, false)?;
    } else if src_ext == "pdf" {
        convert_pdf(source, &out_path, &target)?;
    } else if PANDOC_EXTS.contains(&src_ext.as_str()) {
        convert_via_pandoc(source, &out_path)?;
    } else {
        convert_text(source, &out_path, &src_ext, &target)?;
    }

    let bytes_written = fs::metadata(&out_path)
        .map(|m| m.len())
        .map_err(|e| format!("Failed to stat output {}: {e}", out_path.display()))?;

    Ok(ConvertResult {
        output_path: out_path.to_string_lossy().into_owned(),
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("launchify_{prefix}_{nanos}"));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn supported_targets_covers_all_categories() {
        assert!(supported_targets("png").contains(&"webp"));
        assert!(!supported_targets("png").contains(&"png"));
        assert!(!supported_targets("jpg").contains(&"jpeg"));
        assert!(supported_targets(".JSON").contains(&"yaml"));
        assert_eq!(supported_targets("md"), vec!["html", "txt", "pdf"]);
        assert_eq!(supported_targets("txt"), vec!["base64", "hex", "url"]);
        assert_eq!(supported_targets("base64"), vec!["txt"]);
        assert_eq!(supported_targets("hex"), vec!["txt"]);
        assert_eq!(supported_targets("url"), vec!["txt"]);
        assert_eq!(supported_targets("pdf"), vec!["txt", "png", "jpg"]);
        let mp4_targets = supported_targets("mp4");
        assert!(mp4_targets.contains(&"mkv"));
        assert!(mp4_targets.contains(&"webm"));
        assert!(mp4_targets.contains(&"gif"));
        assert!(mp4_targets.contains(&"mp3"));
        assert!(mp4_targets.contains(&"opus"));
        assert!(!mp4_targets.contains(&"mp4"));
        assert!(supported_targets("docx").contains(&"pdf"));
        assert!(supported_targets("unknown").is_empty());
    }

    #[test]
    fn convert_image_png_to_jpg_and_webp() {
        let dir = TempDir::new("img");
        let src = dir.path().join("sample.png");
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save_with_format(&src, ImageFormat::Png)
            .unwrap();

        let jpg_res = convert_file(&src, "jpg").unwrap();
        assert!(jpg_res.output_path.ends_with("sample.jpg"));
        assert!(jpg_res.bytes_written > 0);
        assert!(Path::new(&jpg_res.output_path).is_file());

        let webp_res = convert_file(&src, "webp").unwrap();
        assert!(webp_res.output_path.ends_with("sample.webp"));
        assert!(webp_res.bytes_written > 0);
    }

    #[test]
    fn convert_data_json_yaml_toml_csv_roundtrip() {
        let dir = TempDir::new("data");
        let json_src = dir.path().join("users.json");
        fs::write(&json_src, r#"[{"name":"Alice","role":"admin"}]"#).unwrap();

        let yaml_res = convert_file(&json_src, "yaml").unwrap();
        assert!(yaml_res.output_path.ends_with("users.yaml"));
        assert!(fs::read_to_string(&yaml_res.output_path)
            .unwrap()
            .contains("Alice"));

        let csv_res = convert_file(&json_src, "csv").unwrap();
        assert!(csv_res.output_path.ends_with("users.csv"));
        let csv_text = fs::read_to_string(&csv_res.output_path).unwrap();
        assert!(csv_text.contains("name,role"));
        assert!(csv_text.contains("Alice,admin"));

        // CSV -> JSON
        let json_from_csv = convert_file(Path::new(&csv_res.output_path), "json").unwrap();
        assert!(json_from_csv.output_path.ends_with("users_converted.json"));
        assert!(fs::read_to_string(&json_from_csv.output_path)
            .unwrap()
            .contains("Alice"));

        // TOML <-> JSON
        let toml_src = dir.path().join("cfg.toml");
        fs::write(&toml_src, "title = \"Launchify\"\ncount = 3\n").unwrap();
        let json_from_toml = convert_file(&toml_src, "json").unwrap();
        let toml_back = convert_file(Path::new(&json_from_toml.output_path), "toml").unwrap();
        assert!(fs::read_to_string(&toml_back.output_path)
            .unwrap()
            .contains("Launchify"));
    }

    #[test]
    fn convert_text_md_base64_hex() {
        let dir = TempDir::new("text");
        let md_src = dir.path().join("note.md");
        fs::write(&md_src, "# Hello\n\nWorld").unwrap();
        let html_res = convert_file(&md_src, "html").unwrap();
        let html = fs::read_to_string(&html_res.output_path).unwrap();
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<p>World</p>"));

        let txt_src = dir.path().join("msg.txt");
        fs::write(&txt_src, "hi!").unwrap();
        let b64_res = convert_file(&txt_src, "base64").unwrap();
        assert_eq!(fs::read_to_string(&b64_res.output_path).unwrap(), "aGkh");

        let txt_back = convert_file(Path::new(&b64_res.output_path), "txt").unwrap();
        assert!(txt_back.output_path.ends_with("msg_converted.txt"));
        assert_eq!(fs::read_to_string(&txt_back.output_path).unwrap(), "hi!");

        let hex_res = convert_file(&txt_src, "hex").unwrap();
        assert_eq!(fs::read_to_string(&hex_res.output_path).unwrap(), "686921");
        let txt_from_hex = convert_file(Path::new(&hex_res.output_path), "txt").unwrap();
        assert_eq!(
            fs::read_to_string(&txt_from_hex.output_path).unwrap(),
            "hi!"
        );
    }

    #[test]
    fn collision_safe_naming_and_invalid_inputs() {
        let dir = TempDir::new("edge");
        let src = dir.path().join("doc.md");
        fs::write(&src, "hi").unwrap();
        fs::write(dir.path().join("doc.html"), "existing").unwrap();

        let res = convert_file(&src, "html").unwrap();
        assert!(res.output_path.ends_with("doc_converted.html"));

        let res2 = convert_file(&src, "html").unwrap();
        assert!(res2.output_path.ends_with("doc_converted_2.html"));

        let hex_non_ascii = dir.path().join("bad.hex");
        fs::write(&hex_non_ascii, "aéb").unwrap();
        assert!(convert_file(&hex_non_ascii, "txt").is_err());

        let missing = dir.path().join("missing.png");
        assert!(convert_file(&missing, "jpg").is_err());
        assert!(convert_file(&src, "png").is_err());
    }

    #[test]
    fn convert_tsv_html_url_and_img_to_pdf() {
        let dir = TempDir::new("ext");

        // TSV <-> JSON
        let tsv_src = dir.path().join("items.tsv");
        fs::write(&tsv_src, "id\tname\n1\tAlpha\n2\tBeta\n").unwrap();
        let json_res = convert_file(&tsv_src, "json").unwrap();
        let json_text = fs::read_to_string(&json_res.output_path).unwrap();
        assert!(json_text.contains("Alpha"));
        assert!(json_text.contains("Beta"));
        let tsv_back = convert_file(Path::new(&json_res.output_path), "tsv").unwrap();
        let tsv_back_text = fs::read_to_string(&tsv_back.output_path).unwrap();
        assert!(tsv_back_text.contains("id\tname"));
        assert!(tsv_back_text.contains("1\tAlpha"));

        // HTML -> TXT
        let html_src = dir.path().join("page.html");
        fs::write(
            &html_src,
            "<html><head><style>body{color:red}</style></head><body><h1>Title</h1><p>Hello &amp; world</p></body></html>",
        )
        .unwrap();
        let txt_from_html = convert_file(&html_src, "txt").unwrap();
        let html_txt = fs::read_to_string(&txt_from_html.output_path).unwrap();
        assert!(html_txt.contains("Title"));
        assert!(html_txt.contains("Hello & world"));
        assert!(!html_txt.contains("color:red"));

        // TXT <-> URL
        let txt_src = dir.path().join("query.txt");
        fs::write(&txt_src, "hello world!").unwrap();
        let url_res = convert_file(&txt_src, "url").unwrap();
        assert_eq!(
            fs::read_to_string(&url_res.output_path).unwrap(),
            "hello%20world%21"
        );
        let txt_from_url = convert_file(Path::new(&url_res.output_path), "txt").unwrap();
        assert_eq!(
            fs::read_to_string(&txt_from_url.output_path).unwrap(),
            "hello world!"
        );

        // IMG -> PDF
        let png_src = dir.path().join("pic.png");
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 128, 255, 255]));
        image::DynamicImage::ImageRgba8(img)
            .save_with_format(&png_src, ImageFormat::Png)
            .unwrap();
        let pdf_res = convert_file(&png_src, "pdf").unwrap();
        assert!(pdf_res.output_path.ends_with("pic.pdf"));
        let pdf_bytes = fs::read(&pdf_res.output_path).unwrap();
        assert!(pdf_bytes.starts_with(b"%PDF-1.4"));
        assert!(pdf_bytes.ends_with(b"%%EOF\n"));
    }
}
