use super::helpers::*;
use super::super::convert::bez_to_wire;
use kurbo::Shape;

#[test]
fn disjoint_union_yields_two_subpaths() {
    let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(2.0, 2.0, 3.0, 3.0), "union");
    assert_eq!(r.subpaths.len(), 2);
}

#[test]
fn difference_with_inner_hole() {
    // a fully contains b; a - b should yield outer ring + inner hole boundary
    let r = run(
        rect(0.0, 0.0, 4.0, 4.0),
        rect(1.0, 1.0, 3.0, 3.0),
        "difference",
    );
    assert!(
        r.subpaths.len() >= 2,
        "expected outer + inner ring, got {}",
        r.subpaths.len()
    );
}

#[test]
fn shared_edge() {
    // Two squares sharing edge x=1 should fuse into a single 2×1 rectangle.
    let r = run(rect(0.0, 0.0, 1.0, 1.0), rect(1.0, 0.0, 2.0, 1.0), "union");
    assert_eq!(r.subpaths.len(), 1);
    let bb = bbox(&r).unwrap();
    assert!(approx_box(bb, (0.0, 0.0, 2.0, 1.0)));
}

#[test]
fn fully_contained_intersection() {
    // a fully contains b → intersection = b
    let r = run(
        rect(0.0, 0.0, 4.0, 4.0),
        rect(1.0, 1.0, 3.0, 3.0),
        "intersection",
    );
    assert_eq!(r.subpaths.len(), 1);
    let bb = bbox(&r).unwrap();
    assert!(approx_box(bb, (1.0, 1.0, 3.0, 3.0)));
}

#[test]
fn cubic_input_circle_via_kurbo() {
    let circ = kurbo::Circle::new((0.0, 0.0), 1.0).to_path(0.01);
    let wire_circle = bez_to_wire(&circ).unwrap();
    let r = run(wire_circle.clone(), wire_circle, "union");
    assert_eq!(r.subpaths.len(), 1);
}

#[test]
fn auto_eps_matches_with_eps_choice() {
    let auto = run(rect(0.0, 0.0, 1.0, 1.0), rect(0.5, 0.5, 1.5, 1.5), "union");
    let with_eps = path_bool(PathBoolArgs {
        a: rect(0.0, 0.0, 1.0, 1.0),
        b: rect(0.5, 0.5, 1.5, 1.5),
        op: "union".into(),
        fill_rule_a: "non-zero".into(),
        fill_rule_b: "non-zero".into(),
        eps: Some(1e-6),
    })
    .unwrap()
    .path;
    assert_eq!(auto.subpaths.len(), with_eps.subpaths.len());
}
