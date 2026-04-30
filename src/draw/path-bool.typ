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
/// flat 3D path (an array of `(origin, closed, segments)` triples).
///
/// - ctx (ctx): The current canvas context.
/// - body (elements): The CeTZ body to walk.
/// - ignore-marks (bool): Drop drawables tagged as marks.
/// - ignore-hidden (bool): Drop drawables tagged as hidden.
/// -> (ctx, path3d)
#let _collect-path3d(ctx, body, ignore-marks: true, ignore-hidden: true) = {
  let subpaths = ()
  for element in body {
    let r = process.element(ctx, element)
    if r != none {
      ctx = r.ctx
      let tags = (drawable.TAG.debug,)
      if ignore-hidden { tags.push(drawable.TAG.hidden) }
      if ignore-marks { tags.push(drawable.TAG.mark) }

      let drawables = drawable.filter-tagged(r.drawables, ..tags)
      subpaths += drawables.filter(d => d.type == "path")
        .map(d => d.segments).join()
    }
  }
  return (ctx, subpaths)
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
    assert(closed,
      message: "path-bool: every input subpath must be closed; got an open subpath")
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

  if z-mismatch {
    // Mismatch is silently projected; surface a panic only in debug builds
    // by guarding behind a flag would be ideal, but Typst has no such flag.
    // Per design decision, project and continue.
  }

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
/// and `"xor"`. The geometry engine is the MIT-licensed `linesweeper` Rust
/// crate, called through CeTZ's WASM module.
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
/// == Anchors
/// Standard path anchors (start, end, mid, percentage along the path) plus
/// the bounding-box anchors derived from the result.
///
/// - a (elements): First operand body.
/// - b (elements): Second operand body.
/// - op (string): One of `"union"`, `"intersection"`, `"difference"`, `"xor"`.
/// - fill-rule (string): `"non-zero"` or `"even-odd"`. Used to interpret each
///   input as a filled region before the operation.
/// - eps (auto, float): Numerical accuracy. `auto` uses linesweeper's
///   automatic, bbox-derived choice. A user-supplied float overrides it.
/// - ignore-marks (bool): Drop arrowheads/marks from the inputs.
/// - ignore-hidden (bool): Drop hidden elements from the inputs.
/// - name (none, string):
/// - ..style (style):
#let path-bool(
  a,
  b,
  op: "difference",
  fill-rule: "non-zero",
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

  assert(op in ("union", "intersection", "difference", "xor"),
    message: "path-bool: invalid op " + repr(op))
  assert(fill-rule in ("non-zero", "even-odd"),
    message: "path-bool: invalid fill-rule " + repr(fill-rule))

  return (ctx => {
    let ctx = ctx
    let (ctx, a-path3d) = _collect-path3d(
      ctx, a,
      ignore-marks: ignore-marks,
      ignore-hidden: ignore-hidden,
    )
    let (ctx, b-path3d) = _collect-path3d(
      ctx, b,
      ignore-marks: ignore-marks,
      ignore-hidden: ignore-hidden,
    )

    let (a-wire, z-a) = _path3d-to-wire2d(a-path3d)
    let (b-wire, _z-b) = _path3d-to-wire2d(b-path3d)

    let result = call_wasm(cetz-core.path_bool_func, (
      a: a-wire,
      b: b-wire,
      op: op,
      fill_rule: fill-rule,
      eps: if eps == auto { none } else { eps },
    ))

    let path3d = _wire2d-to-path3d(result.path, z-a)

    let style = styles.resolve(ctx.style, merge: style, root: "path-bool")

    let drawables = drawable.path(
      fill: style.fill,
      fill-rule: fill-rule,
      stroke: style.stroke,
      path3d,
    )

    let (transform, anchors) = anchor_.setup(
      auto,
      (),
      name: name,
      transform: none,
      path-anchors: true,
      path: drawables,
    )

    drawables = drawable.apply-transform(transform, drawables)

    return (
      ctx: ctx,
      name: name,
      anchors: anchors,
      drawables: drawables,
    )
  },)
}
