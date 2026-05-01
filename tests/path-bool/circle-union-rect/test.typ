#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

#test-case({
  import draw: *
  path-bool(
    { circle((0, 0), radius: 1) },
    { rect((0, -0.5), (2, 0.5)) },
    op: "union",
    fill: orange,
    stroke: black,
  )
})
