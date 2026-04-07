use scad_viewer::wrap_line_pack::{line_count, line_ranges};

#[test]
fn empty_width_list_is_one_line() {
    assert_eq!(line_count(&[], 100.0, 8.0), 1);
    assert!(line_ranges(&[], 100.0, 8.0).is_empty());
}

#[test]
fn two_items_fit_on_one_line() {
    assert_eq!(line_count(&[40.0, 40.0], 100.0, 8.0), 1);
}

#[test]
fn two_items_wrap_to_two_lines() {
    assert_eq!(line_count(&[60.0, 60.0], 100.0, 8.0), 2);
    let ranges = line_ranges(&[60.0, 60.0], 100.0, 8.0);
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0], 0..1);
    assert_eq!(ranges[1], 1..2);
}

#[test]
fn oversize_first_item_still_counts_as_one_line() {
    assert_eq!(line_count(&[200.0, 10.0], 100.0, 8.0), 2);
}
