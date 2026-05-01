use super::helpers::*;

#[test]
fn per_operand_fill_rule_changes_result() {
    // The doubly-wound shape: under non-zero the inner region is filled
    // (winding=2 ≠ 0); under even-odd it's a hole (winding=2, even → outside).
    let a = doubly_wound_annulus();
    let probe = rect(1.5, 1.5, 2.5, 2.5);

    let nz = run_with(a.clone(), probe.clone(), "intersection", "non-zero", "non-zero");
    let eo = run_with(a, probe, "intersection", "even-odd", "non-zero");

    let nz_bb = bbox(&nz).expect("non-zero result should fill the probe");
    assert!(
        approx_box(nz_bb, (1.5, 1.5, 2.5, 2.5)),
        "non-zero intersection should equal the probe; got bbox {nz_bb:?}"
    );
    assert!(
        eo.subpaths.is_empty(),
        "even-odd treats inner area as hole; got {} subpaths",
        eo.subpaths.len()
    );
}

#[test]
fn per_operand_fill_rule_is_independent() {
    // Same semantics but doubly-wound shape is operand b, verifying fill_rule_b
    // is wired up independently.
    let probe = rect(1.5, 1.5, 2.5, 2.5);
    let b = doubly_wound_annulus();

    let nz = run_with(probe.clone(), b.clone(), "intersection", "non-zero", "non-zero");
    let eo = run_with(probe, b, "intersection", "non-zero", "even-odd");

    assert!(!nz.subpaths.is_empty(), "non-zero on b should fill the inner area");
    assert!(
        eo.subpaths.is_empty(),
        "even-odd on b makes inner area a hole; got {} subpaths",
        eo.subpaths.len()
    );
}
