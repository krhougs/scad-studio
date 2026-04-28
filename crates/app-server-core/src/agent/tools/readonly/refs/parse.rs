pub(super) fn source_has_refs_feature(source: &str, feature: &str) -> bool {
    let Some(refs_body) = refs_dict_body(source) else {
        return false;
    };
    dict_body_for_key(refs_body, "features")
        .is_some_and(|features_body| dict_has_key(features_body, feature))
}

fn refs_dict_body(source: &str) -> Option<&str> {
    let mut index = 0;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            index = quoted_string_at(source, index, ch)?.0;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(source, index);
            continue;
        }
        if !source[index..].starts_with("REFS") {
            index += ch.len_utf8();
            continue;
        }
        if !is_identifier_boundary(source, index, "REFS".len()) {
            index += "REFS".len();
            continue;
        }
        let Some(brace_index) = refs_assignment_dict_start(source, index + "REFS".len()) else {
            index += "REFS".len();
            continue;
        };
        return dict_body_at(source, brace_index);
    }
    None
}

fn refs_assignment_dict_start(source: &str, after_refs: usize) -> Option<usize> {
    let assign = skip_inline_ws(source, after_refs);
    if !source[assign..].starts_with('=') || source[assign + 1..].starts_with('=') {
        return None;
    }
    let value = skip_inline_ws(source, assign + 1);
    source[value..].starts_with('{').then_some(value)
}

fn dict_body_for_key<'a>(dict_body: &'a str, key: &str) -> Option<&'a str> {
    let value_start = skip_ws(dict_body, value_start_for_key(dict_body, key)?);
    dict_body_at(dict_body, value_start)
}

fn dict_has_key(dict_body: &str, key: &str) -> bool {
    value_start_for_key(dict_body, key).is_some()
}

fn value_start_for_key(dict_body: &str, key: &str) -> Option<usize> {
    let mut index = 0;
    let mut depth = 0usize;
    while index < dict_body.len() {
        let ch = dict_body[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            let (end, text) = quoted_string_at(dict_body, index, ch)?;
            if depth == 0 && text == key {
                let colon = skip_ws(dict_body, end);
                if dict_body[colon..].starts_with(':') {
                    return Some(colon + 1);
                }
            }
            index = end;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(dict_body, index);
            continue;
        }
        update_depth(ch, &mut depth);
        index += ch.len_utf8();
    }
    None
}

fn dict_body_at(source: &str, open_index: usize) -> Option<&str> {
    if !source[open_index..].starts_with('{') {
        return None;
    }
    let mut index = open_index;
    let mut depth = 0usize;
    let mut content_start = open_index;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            index = quoted_string_at(source, index, ch)?.0;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(source, index);
            continue;
        }
        if ch == '{' {
            if depth == 0 {
                content_start = index + ch.len_utf8();
            }
            depth += 1;
        } else if ch == '}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(&source[content_start..index]);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn quoted_string_at(source: &str, quote_index: usize, quote: char) -> Option<(usize, &str)> {
    let content_start = quote_index + quote.len_utf8();
    let mut escaped = false;
    for (offset, ch) in source[content_start..].char_indices() {
        let index = content_start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some((index + ch.len_utf8(), &source[content_start..index]));
        }
    }
    None
}

fn update_depth(ch: char, depth: &mut usize) {
    match ch {
        '{' | '[' | '(' => *depth += 1,
        '}' | ']' | ')' => *depth = depth.saturating_sub(1),
        _ => {}
    }
}

fn skip_ws(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn skip_inline_ws(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if !matches!(ch, ' ' | '\t') {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn line_comment_end(source: &str, index: usize) -> usize {
    source[index..]
        .find('\n')
        .map_or(source.len(), |offset| index + offset + 1)
}

fn is_identifier_boundary(source: &str, start: usize, len: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[start + len..].chars().next();
    !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
