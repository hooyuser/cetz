use linesweeper::FillRule;

use crate::path_ops::error::PathOpsErr;

pub(crate) fn parse_fill_rule(rule: &str) -> Result<FillRule, PathOpsErr> {
    match rule {
        "non-zero" => Ok(FillRule::NonZero),
        "even-odd" => Ok(FillRule::EvenOdd),
        _ => Err(PathOpsErr::InvalidFillRule(rule.to_string())),
    }
}

pub(crate) fn winding_inside(winding: i32, fill_rule: FillRule) -> bool {
    match fill_rule {
        FillRule::EvenOdd => winding % 2 != 0,
        FillRule::NonZero => winding != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fill_rule_all_valid() {
        assert!(matches!(parse_fill_rule("non-zero"), Ok(FillRule::NonZero)));
        assert!(matches!(parse_fill_rule("even-odd"), Ok(FillRule::EvenOdd)));
    }

    #[test]
    fn parse_fill_rule_invalid() {
        assert!(matches!(
            parse_fill_rule("evenodd"),
            Err(PathOpsErr::InvalidFillRule(_))
        ));
        assert!(matches!(
            parse_fill_rule("nonzero"),
            Err(PathOpsErr::InvalidFillRule(_))
        ));
    }

    #[test]
    fn winding_inside_non_zero() {
        assert!(!winding_inside(0, FillRule::NonZero));
        assert!(winding_inside(1, FillRule::NonZero));
        assert!(winding_inside(-1, FillRule::NonZero));
        assert!(winding_inside(2, FillRule::NonZero));
    }

    #[test]
    fn winding_inside_even_odd() {
        assert!(!winding_inside(0, FillRule::EvenOdd));
        assert!(winding_inside(1, FillRule::EvenOdd));
        assert!(!winding_inside(2, FillRule::EvenOdd));
        assert!(winding_inside(3, FillRule::EvenOdd));
        assert!(!winding_inside(-2, FillRule::EvenOdd));
    }
}
