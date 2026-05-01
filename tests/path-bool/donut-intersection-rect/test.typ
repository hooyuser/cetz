#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

// Build a donut (annulus) by differencing a small disc from a big disc, then
// intersect with a horizontal bar. Exercises hole-bearing input shapes.

#test-case({
  import draw: *

  let donut = path-bool(
    { circle((0, 0), radius: 1.5) },
    { circle((0, 0), radius: 0.7) },
    op: "difference",
    stroke: none,
  )

  path-bool(
    { donut },
    { rect((-2, -0.4), (2, 0.4)) },
    op: "intersection",
    fill: rgb("#A8DADC"),
    stroke: black,
  )
})
