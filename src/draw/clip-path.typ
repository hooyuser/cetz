#import "/src/drawable.typ"
#import "/src/path-util.typ"
#import "/src/process.typ"
#import "/src/styles.typ"
#import "/src/wasm.typ": call_wasm
#import "/src/anchor.typ" as anchor_

#let cetz-core = plugin("/cetz-core/cetz_core.wasm")

#let _path-drawables(drawables, ignore-marks: true, ignore-hidden: true) = {
  let tags = (drawable.TAG.debug, drawable.TAG.content-frame)
  if ignore-hidden { tags.push(drawable.TAG.hidden) }
  if ignore-marks { tags.push(drawable.TAG.mark) }

  let drawables = drawable.filter-tagged(drawables, ..tags)
  return drawables.filter(d => d.type == "path")
}

#let _collect-path-drawables(ctx, operand, label, ignore-marks: true, ignore-hidden: true) = {
  if type(operand) == str {
    assert(
      operand in ctx.nodes,
      message: label + ": no element named " + repr(operand),
    )
    let element = ctx.nodes.at(operand)
    return _path-drawables(
      element.at("drawables", default: ()),
      ignore-marks: ignore-marks,
      ignore-hidden: ignore-hidden,
    )
  }

  let path-drawables = ()
  for element in operand {
    let r = process.element(ctx, element)
    if r != none {
      ctx = r.ctx
      path-drawables += _path-drawables(
        r.drawables,
        ignore-marks: ignore-marks,
        ignore-hidden: ignore-hidden,
      )
    }
  }
  return path-drawables
}

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

#let _path3d-to-wire2d(path3d, label, require-closed: false, tol: 1e-6) = {
  if path3d.len() == 0 {
    return ((subpaths: ()), 0.0)
  }

  let (z0, same-z) = path-util.same-z-plane(path3d, tol: tol)
  assert(same-z, message: label + ": all input vertices must lie in a single z-plane.")

  let drop-z(v) = (v.at(0), v.at(1))

  let wire-subpaths = ()
  for (origin, closed, segments) in path3d {
    if require-closed {
      assert(closed, message: label + ": every clip-region subpath must be closed; got an open subpath")
    }

    let wire-segments = segments.map(seg => {
      let (kind, ..args) = seg
      if kind == "l" {
        (kind: "l", to: drop-z(args.at(0)))
      } else if kind == "c" {
        let (c1, c2, to) = args
        (kind: "c", c1: drop-z(c1), c2: drop-z(c2), to: drop-z(to))
      } else {
        panic(label + ": unsupported path segment kind " + repr(kind))
      }
    })

    wire-subpaths.push((
      origin: drop-z(origin),
      closed: closed,
      segments: wire-segments,
    ))
  }

  return ((subpaths: wire-subpaths), z0)
}

#let _wire2d-to-path3d(wire, z0) = {
  let inflate(v) = (v.at(0), v.at(1), z0)
  return wire.subpaths.map(sp => {
    let segments = sp.segments.map(seg => {
      if seg.kind == "l" {
        ("l", inflate(seg.to))
      } else if seg.kind == "c" {
        ("c", inflate(seg.c1), inflate(seg.c2), inflate(seg.to))
      } else {
        panic("clip-path: unexpected wire segment kind " + repr(seg.kind))
      }
    })
    (inflate(sp.origin), sp.closed, segments)
  })
}

/// Clips one path drawable by a closed clip region.
///
/// `clip-region` may contain multiple closed path drawables. `body` must
/// currently resolve to exactly one path drawable, which may contain both open
/// and closed subpaths.
///
/// - clip-region (elements, str): Closed paths defining the clipping region.
/// - body (elements, str): Exactly one path drawable to clip.
/// - mode (string): `"include"` keeps the part inside the clip region; `"exclude"` keeps the outside.
/// - clip-fill-rule (auto, string): `"non-zero"` or `"even-odd"`, applied to `clip-region`.
/// - body-fill-rule (auto, string): `"non-zero"` or `"even-odd"`, applied to filled closed body subpaths.
/// - eps (auto, float): Numerical accuracy. `auto` uses an automatically determined value.
/// - ignore-marks (bool): Drop marks from the inputs.
/// - ignore-hidden (bool): Drop hidden elements from the inputs.
/// - name (none, string):
#let clip-path(
  clip-region,
  body,
  mode: "include",
  clip-fill-rule: auto,
  body-fill-rule: auto,
  eps: auto,
  ignore-marks: true,
  ignore-hidden: true,
  name: none,
) = {
  assert(
    mode in ("include", "exclude"),
    message: "clip-path: invalid mode " + repr(mode) + ". Expected \"include\" or \"exclude\".",
  )

  let validate-fill-rule(name, value) = {
    assert(
      value == auto or value in ("non-zero", "even-odd"),
      message: "clip-path: invalid " + name + " " + repr(value) + ". Expected `auto`, \"non-zero\", or \"even-odd\".",
    )
  }
  validate-fill-rule("clip-fill-rule", clip-fill-rule)
  validate-fill-rule("body-fill-rule", body-fill-rule)

  return (
    ctx => {
      let clip-drawables = _collect-path-drawables(
        ctx,
        clip-region,
        "clip-path",
        ignore-marks: ignore-marks,
        ignore-hidden: ignore-hidden,
      )
      let body-drawables = _collect-path-drawables(
        ctx,
        body,
        "clip-path",
        ignore-marks: ignore-marks,
        ignore-hidden: ignore-hidden,
      )

      assert(
        body-drawables.len() == 1,
        message: "clip-path: body must resolve to exactly one path drawable; got " + repr(body-drawables.len()),
      )
      let body-drawable = body-drawables.first()

      let clip-path3d = clip-drawables.map(d => d.segments).join(default: ())
      let body-path3d = body-drawable.segments
      let (clip-wire, clip-z) = _path3d-to-wire2d(
        clip-path3d,
        "clip-path",
        require-closed: true,
      )
      let (body-wire, body-z) = _path3d-to-wire2d(
        body-path3d,
        "clip-path",
      )

      if clip-wire.subpaths.len() > 0 and body-wire.subpaths.len() > 0 {
        assert(
          calc.abs(clip-z - body-z) < 1e-6,
          message: "clip-path: input paths must lie in the same z-plane; got z=" + repr(clip-z) + " and z=" + repr(body-z),
        )
      }
      let z0 = if body-wire.subpaths.len() > 0 { body-z } else { clip-z }

      let base-style = styles.resolve(ctx.style)
      let resolved-clip-fill-rule = _infer-fill-rule(
        clip-fill-rule,
        clip-drawables.map(d => d.fill-rule),
        base-style.fill-rule,
      )
      let resolved-body-fill-rule = if body-fill-rule == auto {
        body-drawable.fill-rule
      } else {
        body-fill-rule
      }

      let need-area = body-drawable.fill != none
      let need-line = body-drawable.stroke != none
      if not need-area and not need-line {
        return (
          ctx: ctx,
          name: name,
          anchors: anchor => {
            if anchor == () { () } else {
              panic("clip-path: result is empty; no anchor `" + repr(anchor) + "` available")
            }
          },
          drawables: (),
        )
      }

      let result = call_wasm(cetz-core.clip_path_func, (
        clip_region: clip-wire,
        body: body-wire,
        mode: mode,
        clip_fill_rule: resolved-clip-fill-rule,
        body_fill_rule: resolved-body-fill-rule,
        eps: if eps == auto { none } else { eps },
        need_line: need-line,
        need_area: need-area,
      ))

      let drawables = ()
      if result.area_path != none {
        let path3d = _wire2d-to-path3d(result.area_path, z0)
        if path3d.len() > 0 {
          let d = drawable.path(
            fill: body-drawable.fill,
            fill-rule: body-drawable.fill-rule,
            stroke: none,
            tags: body-drawable.at("tags", default: ()),
            path3d,
          )
          drawables.push(d)
        }
      }
      if result.line_path != none {
        let path3d = _wire2d-to-path3d(result.line_path, z0)
        if path3d.len() > 0 {
          let d = drawable.path(
            fill: none,
            fill-rule: body-drawable.fill-rule,
            stroke: body-drawable.stroke,
            tags: body-drawable.at("tags", default: ()),
            path3d,
          )
          drawables.push(d)
        }
      }

      if drawables.len() == 0 {
        return (
          ctx: ctx,
          name: name,
          anchors: anchor => {
            if anchor == () { () } else {
              panic("clip-path: result is empty; no anchor `" + repr(anchor) + "` available")
            }
          },
          drawables: (),
        )
      }

      let anchor-path = drawables.last()
      let (_, anchors) = anchor_.setup(
        auto,
        (),
        name: name,
        transform: none,
        path-anchors: true,
        path: anchor-path,
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
