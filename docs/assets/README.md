# Laidout README media

`icon.svg` is an editable vector asset. `demo.gif` records real commands and
output, with no UI or API mocks.

Install Python 3, [VHS](https://github.com/charmbracelet/vhs), and its FFmpeg and
TTY dependencies, plus the project's normal build toolchain. From the root:

```sh
python3 scripts/record-readme-demo.py
```

The recorder builds [`examples/readme.rs`](../../examples/readme.rs) through
normal Cargo, then renders annotated JSON at 80 and 38 columns and a Unicode
table at 60 and 14 columns. It verifies that the layouts change while the
Unicode table content survives, and the example checks JSON annotation spans.
The [tape](demo.tape) records actual formatter invocations.

The transcript and working tape are kept in
`~/Documents/Developer/laidout/readme`. `README_RECORD_OUTPUT` overrides that
directory; Cargo continues to use the repository's normal shared build cache.

The [recording script](../../scripts/record-readme-demo.py) publishes `demo.gif`
only after the example and recording finish successfully.
