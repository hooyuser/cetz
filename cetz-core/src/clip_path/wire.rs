use serde::{Deserialize, Serialize};

use crate::path_ops::wire::WirePath;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPathArgs {
    pub clip_region: WirePath,
    pub body: WirePath,
    pub mode: String,
    pub clip_fill_rule: String,
    pub body_fill_rule: String,
    pub eps: Option<f64>,
    pub need_line: bool,
    pub need_area: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPathBatchBody {
    pub body: WirePath,
    pub body_fill_rule: String,
    pub need_line: bool,
    pub need_area: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPathBatchArgs {
    pub clip_region: WirePath,
    pub bodies: Vec<ClipPathBatchBody>,
    pub mode: String,
    pub clip_fill_rule: String,
    pub eps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipPathOutput {
    pub line_path: Option<WirePath>,
    pub area_path: Option<WirePath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipPathBatchOutput {
    pub outputs: Vec<ClipPathOutput>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_ops::wire::{WireSegment, WireSubpath};

    #[test]
    fn round_trip_args_via_cbor() {
        let path = WirePath {
            subpaths: vec![WireSubpath {
                origin: [0.0, 0.0],
                closed: false,
                segments: vec![WireSegment::Line { to: [1.0, 0.0] }],
            }],
        };
        let args = ClipPathArgs {
            clip_region: path.clone(),
            body: path,
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            body_fill_rule: "even-odd".into(),
            eps: None,
            need_line: true,
            need_area: false,
        };

        let mut buf = Vec::new();
        ciborium::ser::into_writer(&args, &mut buf).unwrap();
        let decoded: ClipPathArgs = ciborium::de::from_reader(buf.as_slice()).unwrap();

        assert_eq!(decoded.mode, args.mode);
        assert_eq!(decoded.clip_fill_rule, args.clip_fill_rule);
        assert_eq!(decoded.body_fill_rule, args.body_fill_rule);
        assert_eq!(decoded.need_line, args.need_line);
        assert_eq!(decoded.need_area, args.need_area);
        assert_eq!(decoded.body, args.body);
    }

    #[test]
    fn round_trip_batch_args_via_cbor() {
        let path = WirePath {
            subpaths: vec![WireSubpath {
                origin: [0.0, 0.0],
                closed: false,
                segments: vec![WireSegment::Line { to: [1.0, 0.0] }],
            }],
        };
        let args = ClipPathBatchArgs {
            clip_region: path.clone(),
            bodies: vec![ClipPathBatchBody {
                body: path,
                body_fill_rule: "even-odd".into(),
                need_line: true,
                need_area: false,
            }],
            mode: "include".into(),
            clip_fill_rule: "non-zero".into(),
            eps: None,
        };

        let mut buf = Vec::new();
        ciborium::ser::into_writer(&args, &mut buf).unwrap();
        let decoded: ClipPathBatchArgs = ciborium::de::from_reader(buf.as_slice()).unwrap();

        assert_eq!(decoded.mode, args.mode);
        assert_eq!(decoded.clip_fill_rule, args.clip_fill_rule);
        assert_eq!(decoded.eps, args.eps);
        assert_eq!(decoded.bodies.len(), 1);
        assert_eq!(decoded.bodies[0].body_fill_rule, "even-odd");
        assert!(decoded.bodies[0].need_line);
        assert!(!decoded.bodies[0].need_area);
    }
}
