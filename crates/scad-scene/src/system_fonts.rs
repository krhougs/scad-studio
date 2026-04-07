use std::{collections::HashSet, env, fmt, fs, path::PathBuf, sync::Arc};

use egui::{FontData, FontDefinitions, FontFamily};

/// 检查字体数据中是否包含指定字符的字形
#[allow(dead_code)]
pub fn has_glyph(font_data: &FontData, ch: char) -> bool {
    ttf_parser::Face::parse(font_data.font.as_ref(), font_data.index)
        .ok()
        .and_then(|face| face.glyph_index(ch))
        .is_some()
}

pub fn configure_egui_fonts(ctx: &egui::Context) -> Result<Vec<FontSpec>, SystemFontError> {
    let (fonts, fallback_fonts) = build_font_definitions_for_current_ui()?;
    if fallback_fonts.is_empty() {
        return Ok(Vec::new());
    }
    ctx.set_fonts(fonts);
    Ok(fallback_fonts)
}

pub fn build_font_definitions_for_current_ui()
-> Result<(FontDefinitions, Vec<FontSpec>), SystemFontError> {
    let preferred_languages = preferred_language_tags();
    let primary_fonts = unique_fonts(platform_primary_fonts(&preferred_languages)?);
    let fallback_fonts = unique_fonts(platform_fallback_fonts(&preferred_languages)?);
    let mut fonts = FontDefinitions::default();
    let mut primary_names = Vec::with_capacity(primary_fonts.len());
    for (index, font_spec) in primary_fonts.iter().enumerate() {
        let data = fs::read(&font_spec.path).map_err(|error| {
            SystemFontError(format!(
                "读取系统主字体失败 {}: {error}",
                font_spec.path.display()
            ))
        })?;
        let font_name = format!("system-primary-{index}");
        let mut font_data = FontData::from_owned(data);
        font_data.index = font_spec.index;
        fonts
            .font_data
            .insert(font_name.clone(), Arc::new(font_data));
        primary_names.push(font_name);
    }
    let mut font_names = Vec::with_capacity(fallback_fonts.len());
    for (index, font_spec) in fallback_fonts.iter().enumerate() {
        let data = fs::read(&font_spec.path).map_err(|error| {
            SystemFontError(format!(
                "读取系统字体失败 {}: {error}",
                font_spec.path.display()
            ))
        })?;
        let font_name = format!("system-fallback-{index}");
        let mut font_data = FontData::from_owned(data);
        font_data.index = font_spec.index;
        fonts
            .font_data
            .insert(font_name.clone(), Arc::new(font_data));
        font_names.push(font_name);
    }
    {
        let proportional = fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .expect("egui proportional font family should exist");
        for name in primary_names.iter().rev() {
            proportional.insert(0, name.clone());
        }
        proportional.extend(font_names.iter().cloned());
    }
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .expect("egui monospace font family should exist")
        .extend(font_names);
    Ok((fonts, fallback_fonts))
}

#[allow(dead_code)]
pub fn current_language_preferences() -> Vec<String> {
    preferred_language_tags()
}

fn preferred_language_tags() -> Vec<String> {
    let mut tags = vec!["zh-CN".to_owned()];
    let system_language = detect_language_tag();
    if !tags.iter().any(|tag| tag == &system_language) {
        tags.push(system_language);
    }
    tags
}

pub fn detect_language_tag() -> String {
    ["LC_ALL", "LC_CTYPE", "LANG", "LANGUAGE"]
        .into_iter()
        .filter_map(|name| env::var(name).ok())
        .filter_map(|value| normalize_language_tag(&value))
        .next()
        .unwrap_or_else(|| "en-US".to_owned())
}

fn normalize_language_tag(raw: &str) -> Option<String> {
    let base = raw
        .split('.')
        .next()
        .unwrap_or(raw)
        .split('@')
        .next()
        .unwrap_or(raw)
        .trim();
    if base.is_empty() || base.eq_ignore_ascii_case("c") || base.eq_ignore_ascii_case("posix") {
        return None;
    }

    let normalized = base.replace('_', "-");
    let mut parts = normalized.split('-').filter(|part| !part.is_empty());
    let language = parts.next()?;
    let mut result = language.to_ascii_lowercase();
    if let Some(region) = parts.next() {
        result.push('-');
        result.push_str(&region.to_ascii_uppercase());
    }
    Some(result)
}

fn unique_fonts(fonts: Vec<FontSpec>) -> Vec<FontSpec> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for mut font in fonts {
        if !font.path.is_file() {
            continue;
        }
        font.path = fs::canonicalize(&font.path).unwrap_or(font.path.clone());
        let key = (font.path.clone(), font.index);
        if seen.insert(key) {
            result.push(font);
        }
    }
    result
}

#[cfg(any(test, target_os = "windows"))]
fn font_link_entry_file_name(entry: &str) -> Option<&str> {
    let part = entry.split(',').next()?.trim();
    if part.is_empty() {
        None
    } else {
        Some(part)
    }
}

#[cfg(target_os = "macos")]
fn platform_primary_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    macos::primary_fonts(language_tags)
}

#[cfg(target_os = "linux")]
fn platform_primary_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
fn platform_primary_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    windows::primary_fonts(language_tags)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_primary_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn platform_fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    macos::fallback_fonts(language_tags)
}

#[cfg(target_os = "linux")]
fn platform_fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    linux::fallback_fonts(language_tags)
}

#[cfg(target_os = "windows")]
fn platform_fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    windows::fallback_fonts(language_tags)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_fallback_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    Ok(Vec::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSpec {
    pub path: PathBuf,
    pub index: u32,
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    use core_foundation::{
        base::{CFTypeRef, TCFType},
        string::{CFString, CFStringRef},
    };
    use core_foundation_sys::base::CFRelease;
    use fontdb::{Database, Source};
    use objc2_app_kit::{NSFont, NSFontWeightMedium};

    use super::{FontSpec, SystemFontError};

    type CTFontRef = *const c_void;
    type CTFontDescriptorRef = *const c_void;
    #[allow(dead_code)]
    type CTFontUIFontType = u32;
    type CGFloat = f64;

    #[allow(dead_code)]
    const K_CT_FONT_UIFONT_SYSTEM: CTFontUIFontType = 2;

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        #[allow(dead_code)]
        fn CTFontCreateUIFontForLanguage(
            ui_type: CTFontUIFontType,
            size: CGFloat,
            language: CFStringRef,
        ) -> CTFontRef;
        fn CTFontCreateWithFontDescriptor(
            descriptor: CTFontDescriptorRef,
            size: CGFloat,
            matrix: *const c_void,
        ) -> CTFontRef;
        fn CTFontDescriptorCreateWithNameAndSize(
            name: CFStringRef,
            size: CGFloat,
        ) -> CTFontDescriptorRef;
        fn CTFontCreateForStringWithLanguage(
            current_font: CTFontRef,
            string: CFStringRef,
            range: core_foundation_sys::base::CFRange,
            language: CFStringRef,
        ) -> CTFontRef;
        fn CTFontCopyPostScriptName(font: CTFontRef) -> CFStringRef;
    }

    pub fn primary_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let _ = language_tags;
        let post_script_name = system_medium_post_script_name()?;
        let normalized = normalize_post_script_name(&post_script_name);
        let candidates = [
            post_script_name,
            normalized,
            "SFProText-Medium".to_owned(),
            "HelveticaNeue-Medium".to_owned(),
            "SFProText-Regular".to_owned(),
            "HelveticaNeue".to_owned(),
        ];
        resolve_post_script_fonts(candidates)
    }

    pub fn fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let language_strings = language_tags
            .iter()
            .map(|tag| CFString::new(tag))
            .collect::<Vec<_>>();
        let primary_language = language_strings
            .first()
            .expect("preferred language list should not be empty");
        let font_name = system_medium_post_script_name()?;
        let descriptor_name = CFString::new(&font_name);
        let descriptor = unsafe {
            CTFontDescriptorCreateWithNameAndSize(descriptor_name.as_concrete_TypeRef(), 13.0)
        };
        let font = unsafe { CTFontCreateWithFontDescriptor(descriptor, 13.0, std::ptr::null()) };
        if font.is_null() {
            return Ok(Vec::new());
        }

        let probe_chars = probe_chars(language_tags);
        let mut post_script_names = Vec::new();
        for ch in probe_chars {
            let string = CFString::new(&ch.to_string());
            let fallback_font = unsafe {
                CTFontCreateForStringWithLanguage(
                    font,
                    string.as_concrete_TypeRef(),
                    core_foundation_sys::base::CFRange {
                        location: 0,
                        length: 1,
                    },
                    primary_language.as_concrete_TypeRef(),
                )
            };
            if fallback_font.is_null() {
                continue;
            }
            let attribute = unsafe { CTFontCopyPostScriptName(fallback_font) };
            if !attribute.is_null() {
                let name = unsafe { CFString::wrap_under_create_rule(attribute) };
                post_script_names.push(name.to_string());
            }
            unsafe {
                CFRelease(fallback_font as CFTypeRef);
            }
        }
        unsafe {
            CFRelease(font as CFTypeRef);
        }

        resolve_post_script_fonts(post_script_names)
    }

    fn system_medium_post_script_name() -> Result<String, SystemFontError> {
        let font = unsafe { NSFont::systemFontOfSize_weight(13.0, NSFontWeightMedium) };
        Ok(font.fontName().to_string())
    }

    fn resolve_post_script_fonts(
        post_script_names: impl IntoIterator<Item = String>,
    ) -> Result<Vec<FontSpec>, SystemFontError> {
        let mut database = Database::new();
        database.load_system_fonts();
        let mut fonts = Vec::new();
        for post_script_name in post_script_names {
            let normalized_name = normalize_post_script_name(&post_script_name);
            if let Some(face) = database.faces().find(|face| {
                face.post_script_name == normalized_name
                    || face.post_script_name == post_script_name
            }) {
                let path = match &face.source {
                    Source::File(path) => path.clone(),
                    Source::SharedFile(path, _) => path.clone(),
                    Source::Binary(_) => continue,
                };
                fonts.push(FontSpec {
                    path,
                    index: face.index,
                });
            }
        }
        Ok(fonts)
    }

    fn probe_chars(language_tags: &[String]) -> Vec<char> {
        let primary = language_tags.first().map(String::as_str).unwrap_or("en-US");
        let sample = if primary.starts_with("zh") {
            "文件打开预览等待检测渲染完成错误中文按钮状态"
        } else if primary.starts_with("ja") {
            "日本語表示確認漢字かなカナ"
        } else if primary.starts_with("ko") {
            "한글표시확인미리보기버튼상태"
        } else {
            "PreviewStatus"
        };
        sample.chars().collect()
    }

    fn normalize_post_script_name(name: &str) -> String {
        let trimmed = name.trim_start_matches('.');
        trimmed.replace("UIText", "")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{path::PathBuf, process::Command};

    use super::{FontSpec, SystemFontError, parse_line_list};

    pub fn fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let language_tag = language_tags.first().map(String::as_str).unwrap_or("en-US");
        let pattern = format!("sans:lang={language_tag}");
        let output = Command::new("fc-match")
            .args(["-s", "--format", "%{file}\n", &pattern])
            .output()
            .map_err(|error| SystemFontError(format!("调用 fc-match 失败: {error}")))?;
        if !output.status.success() {
            return Err(SystemFontError(format!(
                "fc-match 返回失败状态: {}",
                output.status
            )));
        }
        Ok(parse_line_list(&String::from_utf8_lossy(&output.stdout))
            .into_iter()
            .map(|path| FontSpec { path, index: 0 })
            .collect())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::env;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_MULTI_SZ, REG_VALUE_TYPE, RegCloseKey,
        RegOpenKeyExW, RegQueryValueExW,
    };

    use super::{FontSpec, SystemFontError, font_link_entry_file_name};

    const FONTLINK_SUBKEY: &str =
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\FontLink\SystemLink";

    pub fn primary_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let dir = fonts_directory();
        let prefer_cjk = language_tags
            .iter()
            .any(|t| t.to_ascii_lowercase().starts_with("zh"));
        let order: &[&str] = if prefer_cjk {
            &["msyh.ttc", "segoeui.ttf"]
        } else {
            &["segoeui.ttf", "msyh.ttc"]
        };
        for name in order {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(vec![FontSpec { path, index: 0 }]);
            }
        }
        Ok(Vec::new())
    }

    pub fn fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let fonts_dir = fonts_directory();
        let prefer_cjk = language_tags
            .iter()
            .any(|t| t.to_ascii_lowercase().starts_with("zh"));
        let mut paths: Vec<PathBuf> = Vec::new();

        if let Ok(hkey) = open_fontlink_key() {
            for key in chain_keys(prefer_cjk) {
                for line in read_font_link_lines(hkey, key) {
                    push_resolved_unique(&mut paths, &line, &fonts_dir);
                }
            }
            unsafe {
                let _ = RegCloseKey(hkey);
            }
        }

        append_well_known(&mut paths, &fonts_dir);

        if paths.is_empty() {
            return Err(SystemFontError(
                "未在 Windows Fonts 目录找到可用字体（FontLink 或常见字体文件）".into(),
            ));
        }

        Ok(paths
            .into_iter()
            .map(|path| FontSpec { path, index: 0 })
            .collect())
    }

    fn fonts_directory() -> PathBuf {
        env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts")
    }

    fn chain_keys(prefer_cjk: bool) -> &'static [&'static str] {
        if prefer_cjk {
            &[
                "Microsoft YaHei UI",
                "Microsoft YaHei",
                "Segoe UI",
            ]
        } else {
            &[
                "Segoe UI",
                "Microsoft YaHei UI",
                "Microsoft YaHei",
            ]
        }
    }

    fn open_fontlink_key() -> Result<HKEY, SystemFontError> {
        let wide: Vec<u16> = FONTLINK_SUBKEY.encode_utf16().chain(Some(0)).collect();
        let mut hkey: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(SystemFontError(format!(
                "打开 FontLink 注册表项失败（错误码 {status}）"
            )));
        }
        Ok(hkey)
    }

    fn read_font_link_lines(hkey: HKEY, value_name: &str) -> Vec<String> {
        let name_w: Vec<u16> = value_name.encode_utf16().chain(Some(0)).collect();
        let mut value_type: REG_VALUE_TYPE = 0;
        let mut byte_len: u32 = 0;
        let q1 = unsafe {
            RegQueryValueExW(
                hkey,
                name_w.as_ptr(),
                std::ptr::null(),
                &mut value_type,
                std::ptr::null_mut(),
                &mut byte_len,
            )
        };
        if q1 != ERROR_SUCCESS || byte_len == 0 {
            return Vec::new();
        }

        let mut buf = vec![0u8; byte_len as usize];
        let q2 = unsafe {
            RegQueryValueExW(
                hkey,
                name_w.as_ptr(),
                std::ptr::null(),
                &mut value_type,
                buf.as_mut_ptr(),
                &mut byte_len,
            )
        };
        if q2 != ERROR_SUCCESS || value_type != REG_MULTI_SZ {
            return Vec::new();
        }

        decode_multisz_utf16(&buf[..byte_len as usize])
    }

    fn decode_multisz_utf16(bytes: &[u8]) -> Vec<String> {
        if bytes.len() < 2 {
            return Vec::new();
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let mut out = Vec::new();
        for chunk in units.split(|&u| u == 0) {
            if chunk.is_empty() {
                continue;
            }
            if let Ok(text) = String::from_utf16(chunk) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_owned());
                }
            }
        }
        out
    }

    fn resolve_font_file_path(file_part: &str, fonts_dir: &Path) -> PathBuf {
        let raw = Path::new(file_part);
        if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            fonts_dir.join(file_part)
        }
    }

    fn contains_same_path(paths: &[PathBuf], candidate: &Path) -> bool {
        let cand = candidate.to_string_lossy().to_ascii_lowercase();
        paths.iter().any(|p| p.to_string_lossy().to_ascii_lowercase() == cand)
    }

    fn push_resolved_unique(paths: &mut Vec<PathBuf>, raw_line: &str, fonts_dir: &Path) {
        let Some(file_part) = font_link_entry_file_name(raw_line) else {
            return;
        };
        let path = resolve_font_file_path(file_part, fonts_dir);
        if path.is_file() && !contains_same_path(paths, &path) {
            paths.push(path);
        }
    }

    fn append_well_known(paths: &mut Vec<PathBuf>, fonts_dir: &Path) {
        for name in [
            "segoeui.ttf",
            "msyh.ttc",
            "seguiemj.ttf",
            "Segoe UI Emoji.ttf",
            "seguiemj.ttc",
        ] {
            let path = fonts_dir.join(name);
            if path.is_file() && !contains_same_path(paths, &path) {
                paths.push(path);
            }
        }
    }
}

#[cfg(any(test, target_os = "linux"))]
fn parse_line_list(output: &str) -> Vec<PathBuf> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[derive(Debug, Clone)]
pub struct SystemFontError(String);

impl std::error::Error for SystemFontError {}

impl fmt::Display for SystemFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{FontSpec, has_glyph, normalize_language_tag, parse_line_list, unique_fonts};

    #[test]
    fn normalize_language_tag_converts_common_locale_formats() {
        assert_eq!(
            normalize_language_tag("zh_CN.UTF-8"),
            Some("zh-CN".to_owned())
        );
        assert_eq!(normalize_language_tag("en_US"), Some("en-US".to_owned()));
        assert_eq!(normalize_language_tag("ja-JP"), Some("ja-JP".to_owned()));
    }

    #[test]
    fn normalize_language_tag_falls_back_for_c_locale() {
        assert_eq!(normalize_language_tag("C"), None);
        assert_eq!(normalize_language_tag("POSIX"), None);
    }

    #[test]
    fn parse_line_list_skips_blank_lines() {
        let parsed = parse_line_list("/tmp/a.ttf\n\n/tmp/b.ttc\n");
        assert_eq!(
            parsed,
            vec![PathBuf::from("/tmp/a.ttf"), PathBuf::from("/tmp/b.ttc")]
        );
    }

    #[test]
    fn font_link_entry_file_name_parses_comma_form() {
        assert_eq!(
            super::font_link_entry_file_name("msyh.ttc,Microsoft YaHei"),
            Some("msyh.ttc")
        );
        assert_eq!(
            super::font_link_entry_file_name("  MSGOTHIC.TTC  , Meiryo"),
            Some("MSGOTHIC.TTC")
        );
        assert_eq!(super::font_link_entry_file_name(""), None);
        assert_eq!(super::font_link_entry_file_name(" , "), None);
    }

    #[test]
    fn unique_fonts_keeps_distinct_faces_of_same_collection() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("scad-studio-font-{suffix}.ttc"));
        fs::write(&path, b"test").expect("temp font file should be created");
        let fonts = unique_fonts(vec![
            FontSpec {
                path: path.clone(),
                index: 0,
            },
            FontSpec {
                path: path.clone(),
                index: 1,
            },
        ]);
        assert_eq!(fonts.len(), 2);
        let _ = fs::remove_file(path);
    }

    /// 关键字符的字形覆盖探测：关闭符号变体、emoji、CJK、基础拉丁
    const PROBE_CHARS: &[char] = &[
        'x',
        '\u{00D7}', // ASCII x, 乘号
        '\u{1F4C1}',
        '\u{2699}', // 文件夹 emoji, 齿轮 emoji
        '文',
        '件',
        '打',
        '开',
        '预',
        '览', // CJK
        'A',
        'a',
        '0', // 基础拉丁
    ];

    #[test]
    fn probe_glyph_coverage_in_system_fonts() {
        let Ok((fonts, _)) = super::build_font_definitions_for_current_ui() else {
            eprintln!("跳过字形覆盖测试：无法加载系统字体");
            return;
        };
        let proportional = match fonts.families.get(&egui::FontFamily::Proportional) {
            Some(names) if !names.is_empty() => names.clone(),
            _ => {
                eprintln!("跳过字形覆盖测试：Proportional family 为空");
                return;
            }
        };

        for &ch in PROBE_CHARS {
            let covered = proportional.iter().any(|name| {
                fonts
                    .font_data
                    .get(name)
                    .is_some_and(|data| has_glyph(data, ch))
            });
            assert!(
                covered,
                "字符 '{ch}' (U+{:04X}) 未被任何 Proportional family 字体覆盖",
                ch as u32,
            );
        }
    }
}
