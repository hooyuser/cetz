#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

// Verify that `..style` is forwarded to the resulting drawable: fill,
// stroke colour, stroke thickness, and stroke dash.

#test-case({
  import draw: *
  path-bool(
    { rect((0, 0), (1.5, 1.5)) },
    { circle((1.5, 1.5), radius: 0.8) },
    op: "difference",
    fill: yellow,
    stroke: (paint: red, thickness: 2pt, dash: "dashed"),
  )
})
