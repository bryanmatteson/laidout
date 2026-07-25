# Contributing to Laidout

Laidout accepts reproducible bug reports, focused design discussions,
documentation corrections, and tested code changes.

## Before opening a change

- Search existing issues and pull requests.
- Open an issue before changing public API or layout semantics. Record the
  compatibility and proof impact there.
- Keep changes narrowly scoped. Do not update generated benchmark evidence or
  frozen baseline code unless the change explicitly requires recapture.

## Local setup

Install the stable Rust toolchain with `rustfmt`, `clippy`, and the
`i686-unknown-linux-gnu` target. The full verifier also requires `z3`.

```sh
rustup component add rustfmt clippy
rustup target add i686-unknown-linux-gnu
./scripts/verify.sh
```

The verifier runs formatting, default and research-feature tests, doctests,
Clippy, 32-bit compilation, strict rustdoc, machine-checked cost laws, and
package construction. Run a focused test while iterating, then the complete
script before submitting.

## Design expectations

- Preserve deterministic output, choice ordering, and typed failure behavior.
- Keep the production kernel iterative and safe for deeply nested documents.
- Add public API documentation and integration coverage for new public items.
- Use the standard library for small implementations. Document every new
  dependency in the pull request.
- Keep the crate independent of any one terminal, formatter, or application
  annotation vocabulary.

## Pull requests

Describe the consumer problem, compatibility impact, tests run, and any proof
or benchmark surfaces affected. New behavior requires regression tests.
By contributing, you agree that your contribution is licensed under this
repository's MIT license.
