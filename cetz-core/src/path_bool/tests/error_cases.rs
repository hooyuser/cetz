use super::helpers::*;

#[test]
fn invalid_op_returns_error() {
    let err = path_bool(PathBoolArgs {
        a: rect(0.0, 0.0, 1.0, 1.0),
        b: rect(0.0, 0.0, 1.0, 1.0),
        op: "subtract".into(),
        fill_rule_a: "non-zero".into(),
        fill_rule_b: "non-zero".into(),
        eps: None,
    })
    .unwrap_err();
    assert!(matches!(err, PathBoolErr::InvalidOp(_)));
}

#[test]
fn invalid_fill_rule_returns_error() {
    let err = path_bool(PathBoolArgs {
        a: rect(0.0, 0.0, 1.0, 1.0),
        b: rect(0.0, 0.0, 1.0, 1.0),
        op: "union".into(),
        fill_rule_a: "evenodd".into(),
        fill_rule_b: "non-zero".into(),
        eps: None,
    })
    .unwrap_err();
    assert!(matches!(err, PathBoolErr::InvalidFillRule(_)));

    let err = path_bool(PathBoolArgs {
        a: rect(0.0, 0.0, 1.0, 1.0),
        b: rect(0.0, 0.0, 1.0, 1.0),
        op: "union".into(),
        fill_rule_a: "non-zero".into(),
        fill_rule_b: "evenodd".into(),
        eps: None,
    })
    .unwrap_err();
    assert!(matches!(err, PathBoolErr::InvalidFillRule(_)));
}

#[test]
fn open_subpath_input_errors() {
    let mut a = rect(0.0, 0.0, 1.0, 1.0);
    a.subpaths[0].closed = false;
    let err = path_bool(PathBoolArgs {
        a,
        b: rect(0.5, 0.5, 1.5, 1.5),
        op: "union".into(),
        fill_rule_a: "non-zero".into(),
        fill_rule_b: "non-zero".into(),
        eps: None,
    })
    .unwrap_err();
    assert!(matches!(err, PathBoolErr::OpenSubpath));
}
