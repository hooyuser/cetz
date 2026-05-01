use super::helpers::*;

#[test]
fn empty_b_truth_table() {
    // union(a, ∅) = a
    let u = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "union");
    assert_eq!(u.subpaths.len(), 1);
    // intersection(a, ∅) = ∅
    let i = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "intersection");
    assert_eq!(i.subpaths.len(), 0);
    // difference(a, ∅) = a
    let d = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "difference");
    assert_eq!(d.subpaths.len(), 1);
    // xor(a, ∅) = a
    let x = run(rect(0.0, 0.0, 1.0, 1.0), empty_path(), "xor");
    assert_eq!(x.subpaths.len(), 1);
}

#[test]
fn empty_both_inputs() {
    for op in &["union", "intersection", "difference", "xor"] {
        let r = run(empty_path(), empty_path(), op);
        assert!(
            r.subpaths.is_empty(),
            "{op} of empty/empty should be empty, got {} subpaths",
            r.subpaths.len()
        );
    }
}
