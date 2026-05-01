#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

#test-case({
  import draw: *

  grid(
    (-3, -2),
    (3, 2),
    step: 1,
    stroke: 0.2pt + gray,
  )

  path-bool(
    {
      compound-path(
        {
          rect((-1, -1), (1, 1))
          circle((0, 0), radius: .5)
        },
        fill: blue,
        fill-rule: "even-odd",
      )
    },
    { rect((2, -2), (3, 1), radius: 0.2, fill: green.desaturate(50%)) },
    op: "union",
    fill: orange.desaturate(40%),
    stroke: black,
    fill-rule: "even-odd"
  )


  // rect((-2, -1), (2, 1), radius: 0.2, fill: green.desaturate(50%))

  // path-bool(
  //   { rect((-2, -1), (2, 1), radius: 0.2) },
  //   { rect((-0.5, -2), (0.5, 0.2), radius: 0.3) },
  //   op: "intersection",
  //   fill: orange.desaturate(40%),
  //   stroke: black,
  // )
})
