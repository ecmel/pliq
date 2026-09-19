# Maintaining documentation

[Developer guide](index.md) · [Documentation home](../index.md)

## Structure

The docs use [mdBook](https://rust-lang.github.io/mdBook/), a documentation
generator written in Rust. Pages are plain Markdown and can also be read
directly in a repository browser or editor. mdBook supplies chapter navigation,
search, heading permalinks, themes, and printable output.

- `docs/index.md` is the main entry point.
- `docs/SUMMARY.md` defines the book's chapters and sidebar order.
- `book.toml` configures the book and its HTML output.
- `docs/user/` explains the implemented language and executable.
- `docs/developer/` explains internals, embedding, and maintenance.
- The root `README.md` is a short introduction and points here for details.

Write the product name as **pliq**, always lowercase. Keep Rust type names
and identifiers in their actual spelling, such as `Interpreter`,
`Table`, and `MutableString`.

## Page conventions

Use one H1 and a navigation line linking to the parent index and related
topics. Add new pages to `docs/SUMMARY.md`, the relevant index, and the main
navigation. Do not add YAML frontmatter. Keep filenames lowercase with hyphens.

Use relative links between documentation pages. Link to headings by their
rendered fragment IDs. Files outside `docs/` use repository URLs under
`https://github.com/ecmel/pliq/blob/main/`, or `tree/main/` for directories,
because they are not part of the Pages site. When moving a page, update incoming
and outgoing links and keep heading fragments consistent.

## Build and preview

Install the same mdBook version used by CI, then run from the repository root:

```sh
cargo install mdbook --version 0.5.4 --locked
mdbook build
mdbook serve --open
```

Alternatively use an official binary from the
[mdBook installation guide](https://rust-lang.github.io/mdBook/guide/installation.html).
Installing the tool requires network access; once installed, the book builds
locally without fetching themes or plugins. The generated site is in
`target/book/`, which is already ignored by Git. `mdbook serve` watches for
changes and reports the local preview address.

[book.toml](https://github.com/ecmel/pliq/blob/main/book.toml) sets `docs/` as the
source and `/pliq/` as the deployed site path. Internal `.md` links are rendered
as HTML links. Rust Playground execution is disabled because the host examples
depend on the local pliq crate; compile them locally as described below.

mdBook is a separate documentation tool; the interpreter does not depend on
mdBook at runtime.

## GitHub Pages

The [documentation workflow](https://github.com/ecmel/pliq/blob/main/.github/workflows/docs.yml)
builds the book on documentation pull requests, relevant pushes to `main`, and
manual runs. It installs the pinned official mdBook binary. Pull requests only
build; pushes and manual runs on `main` upload `target/book/` and deploy through
the `github-pages` environment. Deployment permissions are confined to the
deployment job.

In repository **Settings → Pages**, select **GitHub Actions** as the source.
After that, pushing these changes to `main` can publish the generated site.
Do not select branch-based `/docs` publishing: those files are mdBook source,
not the generated website. Editing local files does not enable Pages or deploy
the site. See the official
[mdBook CI guide](https://rust-lang.github.io/mdBook/continuous-integration.html)
for background.

## Examples

Fenced `pliq` blocks are executable examples. Each block starts with a fresh
environment; statements in a block share bindings. A comment of the form
`// => printed result` specifies an expected display value at that point. Use
separate type-query examples when printed values obscure their types.

Shell commands use `sh` fences and Rust host programs use `rust`.
Keep unsupported or intentionally failing syntax in prose or
tables instead of presenting it as a successful pliq example. Random examples
should demonstrate deterministic properties, such as result length.

Before finishing a documentation change:

1. Evaluate each new pliq block and check the stated results. For a block with
   several expectations, evaluate the source prefix ending at each expectation
   in a fresh interpreter so intermediate results are checked too.
2. Compile and run standalone Rust API examples against the local library.
3. Check documentation links and heading fragments, including links from README.
   Verify that repository URLs point to the corresponding tracked files.
4. Check that every documented operator, built-in, flag, and type matches the
   source and that unsupported argument forms stay explicit.
5. Run `mdbook build` and check links and fragments in the generated HTML.
6. Update the changelog for substantial new documentation or behavior changes.

The language [test suites](testing.md) verify implementation behavior. Prose
examples should describe that behavior and should not imply support for untested
argument forms.
