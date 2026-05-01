#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

#test-case({
  import draw: *
  path-bool(
    { rect((-1, -1), (1, 1)) },
    { circle((0, 0), radius: 0.8) },
    op: "difference",
    fill: blue,
    stroke: black,
  )
})
