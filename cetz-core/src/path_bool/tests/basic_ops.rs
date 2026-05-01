use super::helpers::*;

#[test]
fn union_of_overlapping_squares() {
    let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "union");
    assert_eq!(r.subpaths.len(), 1, "union should be one connected contour");
    let bb = bbox(&r).expect("non-empty result");
    assert!(approx_box(bb, (0.0, 0.0, 1.5, 1.5)), "union bbox: {bb:?}");
    assert!(approx_eq(linear_area(&r), 1.75), "union area: {}", linear_area(&r));
}

#[test]
fn intersection_of_overlapping_squares() {
    let r = run(
        rect(0.0, 0.0, 1.0, 1.0),
        rect(0.5, 0.5, 1.5, 1.5),
        "intersection",
    );
    assert_eq!(r.subpaths.len(), 1);
    let bb = bbox(&r).unwrap();
    assert!(approx_box(bb, (0.5, 0.5, 1.0, 1.0)), "intersection bbox: {bb:?}");
    assert!(approx_eq(linear_area(&r), 0.25), "intersection area: {}", linear_area(&r));
}

#[test]
fn difference_of_overlapping_squares() {
    let r = run(
        rect(0.0, 0.0, 1.0, 1.0),
        rect(0.5, 0.5, 1.5, 1.5),
        "difference",
    );
    assert_eq!(r.subpaths.len(), 1);
    let bb = bbox(&r).unwrap();
    assert!(approx_box(bb, (0.0, 0.0, 1.0, 1.0)), "diff bbox: {bb:?}");
    assert!(approx_eq(linear_area(&r), 0.75), "difference area: {}", linear_area(&r));
}

#[test]
fn xor_of_overlapping_squares() {
    let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "xor");
    assert!(r.subpaths.len() >= 1);
    let bb = bbox(&r).unwrap();
    assert!(approx_box(bb, (0.0, 0.0, 1.5, 1.5)), "xor bbox: {bb:?}");
    // XOR area = |A| + |B| - 2·|A∩B| = 1 + 1 - 2·0.25 = 1.5
    assert!(approx_eq(linear_area(&r), 1.5), "xor area: {}", linear_area(&r));
}
