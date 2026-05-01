#import "/src/drawable.typ"
#import "/src/path-util.typ"
#import "/src/process.typ"
#import "/src/styles.typ"
#import "/src/wasm.typ": call_wasm
#import "/src/anchor.typ" as anchor_

#let cetz-core = plugin("/cetz-core/cetz_core.wasm")

// =============================================================================
// Internal helpers
// =============================================================================

/// Step C1: Run a CeTZ body through `process.element`, filter marks/hidden
/// drawables, and collect every path drawable's subpaths into a single
/// flat 3D path (an array of `(origin, closed, segments)` triples), along
/// with the set of `fill-rule` values that the contributing path drawables
/// carried — used downstream by `_infer-fill-rule` to mimic the implicit
/// `compound-path` that wraps each operand.
///
/// - ctx (ctx): The current canvas context.
/// - body (elements): The CeTZ body to walk.
/// - ignore-marks (bool): Drop drawables tagged as marks.
/// - ignore-hidden (bool): Drop drawables tagged as hidden.
/// -> (ctx, path3d, fill-rules)
#let _collect-path3d(ctx, body, ignore-marks: true, ignore-hidden: true) = {
  let subpaths = ()
  let fill-rules = ()
  for element in body {
    let r = process.element(ctx, element)
    if r != none {
      ctx = r.ctx
      let tags = (drawable.TAG.debug,)
      if ignore-hidden { tags.push(drawable.TAG.hidden) }
      if ignore-marks { tags.push(drawable.TAG.mark) }

      let drawables = drawable.filter-tagged(r.drawables, ..tags)
      let path-drawables = drawables.filter(d => d.type == "path")
      subpaths += path-drawables.map(d => d.segments).join()
      fill-rules += path-drawables.map(d => d.fill-rule)
    }
  }
  return (ctx, subpaths, fill-rules)
}

/// Pick a fill-rule for one operand, mimicking how a hypothetical
/// `compound-path(operand)` wrapper would resolve it:
///
/// - If the user passed an explicit value (not `auto`), use it.
/// - Else if every contributing path drawable agrees on a single
///   fill-rule, inherit that one.
/// - Else fall back to the path-bool element's own resolved style — same
///   as `compound-path` defaulting to the style root when no explicit
///   `fill-rule:` is given.
///
/// - arg (auto, string): The user-supplied `fill-rule-a` / `fill-rule-b`.
/// - observed (array): Fill-rules seen across the operand's drawables.
/// - default (string): Style-resolved fallback.
/// -> string
#let _infer-fill-rule(arg, observed, default) = {
  if arg != auto {
    return arg
  }
  let unique = observed.dedup()
  if unique.len() == 1 {
    return unique.first()
  }
  return default
}

/// Step C2: Project a CeTZ 3D path to a 2D wire path. Captures the z value
/// of the first vertex and asserts every other vertex shares it (within
/// `tol`). Open subpaths are rejected immediately.
///
/// - path3d (path3d): The CeTZ 3D path.
/// - tol (float): Tolerance for z-coplanarity.
/// -> (wire-path, z0)
#let _path3d-to-wire2d(path3d, tol: 1e-6) = {
  if path3d.len() == 0 {
    return ((subpaths: ()), 0.0)
  }

  let z0 = path3d.first().at(0).at(2)
  let z-mismatch = false

  let drop-z(v) = (v.at(0), v.at(1))
  let check-z(v) = {
    if calc.abs(v.at(2) - z0) > tol { z-mismatch = true }
  }

  let wire-subpaths = ()
  for (origin, closed, segments) in path3d {
    assert(closed, message: "path-bool: every input subpath must be closed; got an open subpath")
    check-z(origin)

    let wire-segments = segments.map(seg => {
      let (kind, ..args) = seg
      if kind == "l" {
        let to = args.at(0)
        check-z(to)
        (kind: "l", to: drop-z(to))
      } else if kind == "c" {
        let (c1, c2, to) = args
        check-z(c1)
        check-z(c2)
        check-z(to)
        (kind: "c", c1: drop-z(c1), c2: drop-z(c2), to: drop-z(to))
      } else {
        panic("path-bool: unsupported path segment kind " + repr(kind))
      }
    })

    wire-subpaths.push((
      origin: drop-z(origin),
      closed: closed,
      segments: wire-segments,
    ))
  }

  assert.eq(z-mismatch, false, message: "path-bool: all input vertices must lie in a single z-plane.")

  return ((subpaths: wire-subpaths), z0)
}

/// Step C2 (inverse): inject z0 back into a 2D wire path to produce a CeTZ
/// 3D path.
///
/// - wire (wire-path): The 2D wire path.
/// - z0 (float): The z value to assign to every vertex.
/// -> path3d
#let _wire2d-to-path3d(wire, z0) = {
  let inflate(v) = (v.at(0), v.at(1), z0)
  return wire.subpaths.map(sp => {
    let segments = sp.segments.map(seg => {
      if seg.kind == "l" {
        ("l", inflate(seg.to))
      } else if seg.kind == "c" {
        ("c", inflate(seg.c1), inflate(seg.c2), inflate(seg.to))
      } else {
        panic("path-bool: unexpected wire segment kind " + repr(seg.kind))
      }
    })
    (inflate(sp.origin), sp.closed, segments)
  })
}

// =============================================================================
// Public draw function
// =============================================================================

/// Performs a boolean operation on the paths produced by two CeTZ bodies.
/// The supported operations are `"union"`, `"intersection"`, `"difference"`,
/// and `"xor"`. The geometry engine is the `linesweeper` Rust crate,
/// called through CeTZ's WASM module.
///
/// ```example
/// path-bool(
///   { rect((-1, -1), (1, 1)) },
///   { circle((0, 0), radius: 0.8) },
///   op: "difference",
///   fill: blue,
/// )
/// ```
///
/// All input subpaths must be closed and lie in a single z-plane. The output
/// is a single path drawable in the z-plane of the first input.
///
/// Each operand has its own fill-rule, which decides how its self-overlapping
/// or nested subpaths are interpreted as a filled region *before* the
/// boolean operation runs. By default (`auto`) the fill-rule is inferred
/// from the operand: if every path drawable produced by the body agrees on
/// one fill-rule (e.g. the body is a single `compound-path(..., fill-rule:
/// "even-odd")`), that value is used; otherwise it falls back to
/// `path-bool`'s own resolved style (same fallback `compound-path` itself
/// uses).
///
/// == Anchors
/// Standard path anchors (start, end, mid, percentage along the path) plus
/// the bounding-box anchors derived from the result.
///
/// - a (elements): First operand body.
/// - b (elements): Second operand body.
/// - op (string): One of `"union"`, `"intersection"`, `"difference"`, `"xor"`.
/// - fill-rule-a (auto, string): `"non-zero"` or `"even-odd"`, applied to `a`'s
///   winding number. `auto` infers from the operand (see above).
/// - fill-rule-b (auto, string): Same as `fill-rule-a`, but for `b`.
/// - eps (auto, float): Numerical accuracy. `auto` uses an automatic,
///   bbox-derived choice (matching `linesweeper::binary_op`'s default). A
///   user-supplied float overrides it.
/// - ignore-marks (bool): Drop arrowheads/marks from the inputs.
/// - ignore-hidden (bool): Drop hidden elements from the inputs.
/// - name (none, string):
/// - ..style (style):
#let path-bool(
  a,
  b,
  op: "difference",
  fill-rule-a: auto,
  fill-rule-b: auto,
  eps: auto,
  ignore-marks: true,
  ignore-hidden: true,
  name: none,
  ..style,
) = {
  assert.eq(
    style.pos(),
    (),
    message: "Unexpected positional arguments: " + repr(style.pos()),
  )
  let style = style.named()

  assert(
    op in ("union", "intersection", "difference", "xor"),
    message: "path-bool: invalid op "
      + repr(op)
      + ". Expected one of: \"union\", \"intersection\", \"difference\", \"xor\"",
  )

  let validate-fill-rule(name, value) = {
    assert(
      value == auto or value in ("non-zero", "even-odd"),
      message: "path-bool: invalid " + name + " " + repr(value)
        + ". Expected `auto`, \"non-zero\", or \"even-odd\".",
    )
  }
  validate-fill-rule("fill-rule-a", fill-rule-a)
  validate-fill-rule("fill-rule-b", fill-rule-b)

  return (
    ctx => {
      let (_, a-path3d, a-fill-rules) = _collect-path3d(
        ctx,
        a,
        ignore-marks: ignore-marks,
        ignore-hidden: ignore-hidden,
      )
      let (_, b-path3d, b-fill-rules) = _collect-path3d(
        ctx,
        b,
        ignore-marks: ignore-marks,
        ignore-hidden: ignore-hidden,
      )

      let (a-wire, az) = _path3d-to-wire2d(a-path3d)
      let (b-wire, bz) = _path3d-to-wire2d(b-path3d)

      assert(
        calc.abs(az - bz) < 1e-6,
        message: "path-bool: input paths must lie in the same z-plane; got z=" + repr(az) + " and z=" + repr(bz),
      )

      let resolved-style = styles.resolve(ctx.style, merge: style, root: "path-bool")
      let resolved-fill-rule-a = _infer-fill-rule(fill-rule-a, a-fill-rules, resolved-style.fill-rule)
      let resolved-fill-rule-b = _infer-fill-rule(fill-rule-b, b-fill-rules, resolved-style.fill-rule)

      let result = call_wasm(cetz-core.path_bool_func, (
        a: a-wire,
        b: b-wire,
        op: op,
        fill_rule_a: resolved-fill-rule-a,
        fill_rule_b: resolved-fill-rule-b,
        eps: if eps == auto { none } else { eps },
      ))

      let path3d = _wire2d-to-path3d(result.path, az)

      // Empty result (e.g. difference of identical shapes): emit no drawables.
      if path3d.len() == 0 {
        return (
          ctx: ctx,
          name: name,
          anchors: anchor => {
            if anchor == () { () } else {
              panic("path-bool: result is empty; no anchor `" + repr(anchor) + "` available")
            }
          },
          drawables: (),
        )
      }

      let drawables = drawable.path(
        fill: resolved-style.fill,
        fill-rule: resolved-style.fill-rule,
        stroke: resolved-style.stroke,
        path3d,
      )

      let (_, anchors) = anchor_.setup(
        auto,
        (),
        name: name,
        transform: none,
        path-anchors: true,
        path: drawables,
      )

      return (
        ctx: ctx,
        name: name,
        anchors: anchors,
        drawables: drawables,
      )
    },
  )
}
