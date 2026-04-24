use app_server_protocol::{PathHandle, PathHandleValidationError, WorkspaceId};

#[test]
fn path_handle_rejects_dot_dot_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["src", ".."])
        .expect_err("dot dot should be rejected");
    assert_eq!(error, PathHandleValidationError::DotDotSegment);
}

#[test]
fn path_handle_rejects_single_dot_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["."]).expect_err("single dot should fail");
    assert_eq!(error, PathHandleValidationError::SingleDotSegment);
}

#[test]
fn path_handle_rejects_empty_segment() {
    let error = PathHandle::new(WorkspaceId::new("ws"), [""])
        .expect_err("empty segment should be rejected");
    assert_eq!(error, PathHandleValidationError::EmptySegment);
}

#[test]
fn path_handle_rejects_native_separator() {
    let error = PathHandle::new(WorkspaceId::new("ws"), ["src/main.rs"])
        .expect_err("slash should be rejected");
    assert_eq!(error, PathHandleValidationError::NativeSeparator);

    let error = PathHandle::new(WorkspaceId::new("ws"), [r"src\main.rs"])
        .expect_err("backslash should be rejected");
    assert_eq!(error, PathHandleValidationError::NativeSeparator);
}

#[test]
fn path_handle_nfc_canonical_equivalent() {
    let composed = PathHandle::new(WorkspaceId::new("ws"), ["café"]).unwrap();
    let decomposed = PathHandle::new(WorkspaceId::new("ws"), ["cafe\u{301}"]).unwrap();
    assert_eq!(composed, decomposed);
}

#[test]
fn path_handle_accepts_cjk_emoji_and_common_js_project_names() {
    for segments in [
        vec!["模型.scad"],
        vec!["零件", "支架👍🏽.scad"],
        vec!["@types", "node", "index.d.ts"],
        vec!["app", "[[...slug]]", "page.tsx"],
        vec!["app", "(marketing)", "page.tsx"],
        vec!["src", "routes", "+page.svelte"],
        vec!["routes", "concerts.$city.tsx"],
    ] {
        PathHandle::new(WorkspaceId::new("ws"), segments).expect("portable path should be valid");
    }
}

#[test]
fn path_handle_rejects_reserved_names_and_problem_symbols() {
    let cases = [
        ("CON.scad", PathHandleValidationError::WindowsReservedName),
        ("NUL.tar.gz", PathHandleValidationError::WindowsReservedName),
        (
            "支架：左.scad",
            PathHandleValidationError::DisallowedCharacter,
        ),
        (
            "foo#bar.scad",
            PathHandleValidationError::DisallowedCharacter,
        ),
        (
            "a\u{200d}b.scad",
            PathHandleValidationError::DisallowedCharacter,
        ),
        ("name.", PathHandleValidationError::TrailingSpaceOrDot),
        (" name.scad", PathHandleValidationError::LeadingDisallowed),
        ("..hidden", PathHandleValidationError::DotDotSegment),
    ];

    for (segment, expected) in cases {
        let error = PathHandle::new(WorkspaceId::new("ws"), [segment])
            .expect_err("invalid segment should fail");
        assert_eq!(error, expected, "{segment}");
    }
}

#[test]
fn relative_links_resolve_to_canonical_portable_path() {
    let base = PathHandle::new(WorkspaceId::new("ws"), ["docs", "guide.md"]).unwrap();

    let resolved = PathHandle::resolve_relative_link(&base, "../模型/%E9%9B%B6件.scad")
        .expect("relative link should resolve");

    assert_eq!(resolved.path_segments(), ["模型", "零件.scad"]);
}

#[test]
fn relative_links_reject_escape_absolute_url_and_query() {
    let base = PathHandle::new(WorkspaceId::new("ws"), ["docs"]).unwrap();

    for link in [
        "../../secret.scad",
        "/abs.scad",
        "C:/tmp/a.scad",
        "https://example.test/a",
        "a.scad?x=1",
    ] {
        PathHandle::resolve_relative_link(&base, link).expect_err("invalid link should fail");
    }
}

#[test]
fn case_fold_key_matches_case_insensitive_conflicts() {
    let upper = PathHandle::new(WorkspaceId::new("ws"), ["Cube.scad"]).unwrap();
    let lower = PathHandle::new(WorkspaceId::new("ws"), ["cube.scad"]).unwrap();

    assert_eq!(upper.case_fold_key(), lower.case_fold_key());
}
