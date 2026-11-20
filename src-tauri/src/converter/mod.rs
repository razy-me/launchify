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

const IMAGE_FORMATS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "ico", "tiff"];
const DATA_FORMATS: &[&str] = &["json", "yaml", "toml", "csv"];
const MEDIA_FORMATS: &[&str] = &["mp4", "mp3", "wav", "webm", "mkv", "ogg", "flac"];

fn normalize_ext(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_ascii_lowercase()
}

/// Returns supported target extensions for a given source file extension.
pub fn supported_targets(ext: &str) -> Vec<&'static str> {
    let norm = normalize_ext(ext);
    if norm.is_empty() {
        return Vec::new();
    }

    if IMAGE_FORMATS.contains(&norm.as_str()) {
        return IMAGE_FORMATS
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
    }

    if DATA_FORMATS.contains(&norm.as_str()) || norm == "yml" {
        return DATA_FORMATS
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
        "md" | "markdown" => return vec!["html"],
        "txt" => return vec!["base64", "hex"],
        "base64" | "b64" => return vec!["txt"],
        "hex" => return vec!["txt"],
        _ => {}
    }

    if MEDIA_FORMATS.contains(&norm.as_str()) {
        return MEDIA_FORMATS
            .iter()
            .copied()
            .filter(|&f| f != norm)
            .collect();
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
    parent.join(format!("{stem}_converted.{target_ext}"))
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
        _ => None,
    }
}

fn convert_image(source: &Path, out_path: &Path, target_ext: &str) -> Result<(), String> {
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
        "csv" => {
            let mut reader = csv::Reader::from_reader(content.as_bytes());
            let headers = reader
                .headers()
                .map_err(|e| format!("Invalid CSV headers: {e}"))?
                .clone();
            let mut rows = Vec::new();
            for result in reader.records() {
                let record = result.map_err(|e| format!("Invalid CSV row: {e}"))?;
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

fn write_json_as_csv(val: &serde_json::Value, out_path: &Path) -> Result<(), String> {
    let rows = val
        .as_array()
        .ok_or_else(|| "CSV export requires a JSON/YAML/TOML array of flat objects".to_string())?;
    if rows.is_empty() {
        return Err("Cannot export empty array to CSV".to_string());
    }

    let mut header_set = BTreeSet::new();
    let mut headers = Vec::new();
    for row in rows {
        let obj = row
            .as_object()
            .ok_or_else(|| "CSV export requires every array element to be an object".to_string())?;
        for key in obj.keys() {
            if header_set.insert(key.clone()) {
                headers.push(key.clone());
            }
        }
    }

    let mut writer = csv::Writer::from_path(out_path)
        .map_err(|e| format!("Failed to create CSV {}: {e}", out_path.display()))?;
    writer
        .write_record(&headers)
        .map_err(|e| format!("Failed to write CSV headers: {e}"))?;

    for row in rows {
        let obj = row.as_object().expect("validated above");
        let record: Vec<String> = headers
            .iter()
            .map(|h| obj.get(h).map(json_scalar_to_string).unwrap_or_default())
            .collect();
        writer
            .write_record(&record)
            .map_err(|e| format!("Failed to write CSV row: {e}"))?;
    }

    writer
        .flush()
        .map_err(|e| format!("Failed to flush CSV {}: {e}", out_path.display()))
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
        "csv" => return write_json_as_csv(&val, out_path),
        _ => return Err(format!("Unsupported data target format: {target_ext}")),
    };
    fs::write(out_path, serialized)
        .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
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
        _ => Err(format!(
            "Unsupported text conversion: {src_ext} -> {target_ext}"
        )),
    }
}

fn convert_media(source: &Path, out_path: &Path) -> Result<(), String> {
    let output = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(source)
        .arg(out_path)
        .output()
        .map_err(|e| {
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

    if IMAGE_FORMATS.contains(&src_ext.as_str()) {
        convert_image(source, &out_path, &target)?;
    } else if DATA_FORMATS.contains(&src_ext.as_str()) || src_ext == "yml" {
        convert_data(source, &out_path, &src_ext, &target)?;
    } else if MEDIA_FORMATS.contains(&src_ext.as_str()) {
        convert_media(source, &out_path)?;
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
        assert_eq!(supported_targets("md"), vec!["html"]);
        assert_eq!(supported_targets("txt"), vec!["base64", "hex"]);
        assert_eq!(supported_targets("base64"), vec!["txt"]);
        assert_eq!(supported_targets("hex"), vec!["txt"]);
        assert!(supported_targets("mp4").contains(&"mp3"));
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

        let missing = dir.path().join("missing.png");
        assert!(convert_file(&missing, "jpg").is_err());
        assert!(convert_file(&src, "png").is_err());
    }
}
