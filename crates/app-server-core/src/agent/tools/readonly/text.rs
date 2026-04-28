pub(super) fn is_probably_binary(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .any(|byte| *byte == 0 || (*byte < 0x20 && !matches!(*byte, b'\n' | b'\r' | b'\t')))
}
