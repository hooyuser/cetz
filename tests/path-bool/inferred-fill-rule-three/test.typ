#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

// Verify that path-bool inherits each operand's fill-rule when set to
// `auto` (the default). The operand `a` here is a compound-path with two
// concentric, same-orientation rectangles — under "non-zero" linesweeper
// treats it as a solid square (the inner ring is doubly wound, still
// non-zero), but under "even-odd" the inner area is a hole. Intersecting
// with a horizontal bar through the centre therefore yields visibly
// different results.
//
// Three canvases, side-by-side:
//   1. fill-rule "non-zero" inferred from a plain compound-path → solid bar.
//   2. fill-rule "even-odd" inferred from `compound-path(..., fill-rule:
//      "even-odd")` → bar with a notch where the hole is.
//   3. Same as (2) but the user explicitly forces fill-rule-a: "non-zero",
//      overriding inference, so it should look like (1).

#test-case({
  import draw: *

  let nested = {
    rect((-1.5, -1.5), (1.5, 1.5))
    rect((-0.6, -0.6), (0.6, 0.6))
  }

  // Panel 1: implicit non-zero (style root default).
  path-bool(
    compound-path(nested),
    { rect((-2, -0.3), (2, 0.3)) },
    op: "intersection",
    fill: rgb("#A8DADC"),
    stroke: black,
  )

  set-origin((4, 0))

  // Panel 2: even-odd inferred from the compound-path on operand a.
  path-bool(
    compound-path(nested, fill-rule: "even-odd"),
    { rect((-2, -0.3), (2, 0.3)) },
    op: "intersection",
    fill: rgb("#A8DADC"),
    stroke: black,
  )

  set-origin((4, 0))

  // Panel 3: explicit fill-rule-a overrides inference back to non-zero.
  path-bool(
    compound-path(nested, fill-rule: "even-odd"),
    { rect((-2, -0.3), (2, 0.3)) },
    op: "intersection",
    fill-rule-a: "non-zero",
    fill: rgb("#A8DADC"),
    stroke: black,
  )
})
