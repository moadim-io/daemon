# moadim

npm distribution package for the `moadim` daemon CLI.

This package does not compile Moadim during install. It resolves the matching
prebuilt platform package from optional dependencies and runs that binary:

```sh
npm install -g moadim
moadim --version
```

If installation omitted optional dependencies (`npm install --omit=optional`) or
the current platform is unsupported, the wrapper prints a clear error and points
to GitHub Releases for direct downloads.
