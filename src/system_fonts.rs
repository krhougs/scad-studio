use std::{
    collections::HashSet,
    env, fmt, fs,
    path::PathBuf,
    sync::Arc,
};

use egui::{FontData, FontDefinitions, FontFamily};

pub fn configure_egui_fonts(ctx: &egui::Context) -> Result<Vec<FontSpec>, SystemFontError> {
    let (fonts, fallback_fonts) = build_font_definitions_for_current_ui()?;
    if fallback_fonts.is_empty() {
        return Ok(Vec::new());
    }
    ctx.set_fonts(fonts);
    Ok(fallback_fonts)
}

pub fn build_font_definitions_for_current_ui(
) -> Result<(FontDefinitions, Vec<FontSpec>), SystemFontError> {
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
    let Some(language) = parts.next() else {
        return None;
    };
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

#[cfg(target_os = "macos")]
fn platform_primary_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    macos::primary_fonts(language_tags)
}

#[cfg(target_os = "linux")]
fn platform_primary_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
fn platform_primary_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
    Ok(Vec::new())
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
    type CTFontUIFontType = u32;
    type CGFloat = f64;

    const K_CT_FONT_UIFONT_SYSTEM: CTFontUIFontType = 2;

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
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
            if let Some(face) = database
                .faces()
                .find(|face| {
                    face.post_script_name == normalized_name || face.post_script_name == post_script_name
                })
            {
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

    use super::{parse_line_list, FontSpec, SystemFontError};

    pub fn fallback_fonts(language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let language_tag = language_tags
            .first()
            .map(String::as_str)
            .unwrap_or("en-US");
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
    use std::{env, path::PathBuf, process::Command};

    use super::{FontSpec, SystemFontError};

    pub fn fallback_fonts(_language_tags: &[String]) -> Result<Vec<FontSpec>, SystemFontError> {
        let base_family = query_message_font_family()?;
        let entries = query_system_link_entries(&base_family)?;
        let fonts_dir = env::var("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        Ok(entries
            .into_iter()
            .filter_map(|entry| entry.split(',').next().map(str::trim).map(PathBuf::from))
            .map(|path| if path.is_absolute() { path } else { fonts_dir.join(path) })
            .map(|path| FontSpec { path, index: 0 })
            .collect())
    }

    fn query_message_font_family() -> Result<String, SystemFontError> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[System.Drawing.SystemFonts]::MessageBoxFont.Name",
            ])
            .output()
            .map_err(|error| SystemFontError(format!("读取系统消息字体失败: {error}")))?;
        if !output.status.success() {
            return Err(SystemFontError(format!(
                "读取系统消息字体返回失败状态: {}",
                output.status
            )));
        }
        let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if name.is_empty() {
            return Err(SystemFontError("系统消息字体名称为空".into()));
        }
        Ok(name)
    }

    fn query_system_link_entries(base_family: &str) -> Result<Vec<String>, SystemFontError> {
        let script = format!(
            "$props=Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\FontLink\\SystemLink'; $value=$props.PSObject.Properties['{base_family}'].Value; if ($value) {{ $value }}"
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|error| SystemFontError(format!("读取 Windows 字体回退链失败: {error}")))?;
        if !output.status.success() {
            return Err(SystemFontError(format!(
                "读取 Windows 字体回退链返回失败状态: {}",
                output.status
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect())
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
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

    use super::{normalize_language_tag, parse_line_list, unique_fonts, FontSpec};

    #[test]
    fn normalize_language_tag_converts_common_locale_formats() {
        assert_eq!(normalize_language_tag("zh_CN.UTF-8"), Some("zh-CN".to_owned()));
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
        assert_eq!(parsed, vec![PathBuf::from("/tmp/a.ttf"), PathBuf::from("/tmp/b.ttc")]);
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
}
