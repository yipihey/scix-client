# Packaging

This directory holds templates that live in *other* repositories. They are
checked in here so the canonical version lives next to the source, and so the
release workflow can bump them automatically.

## Homebrew (`homebrew/scix-client.rb`)

One-time setup:

1. Create a public repo `github.com/yipihey/homebrew-scix`.
2. Copy `homebrew/scix-client.rb` to `Formula/scix-client.rb` in that repo.
3. Fill in the three `sha256` placeholders from the v0.3.1 release tarballs
   (`shasum -a 256 scix-v0.3.1-*.tar.gz`).
4. Push.
5. Create a fine-grained PAT with `Contents: read+write` on the tap repo, add
   it as repository secret `HOMEBREW_TAP_TOKEN` on `yipihey/scix-client`.

After that, the `homebrew` job in `.github/workflows/release.yml` opens a PR on
the tap repo for every release. Users install with:

```bash
brew install yipihey/scix/scix-client
```

## conda-forge (`conda-forge/meta.yaml`)

One-time setup:

1. Fork `github.com/conda-forge/staged-recipes`.
2. Create a directory `recipes/scix-client/` and copy `conda-forge/meta.yaml`
   into it.
3. Fill in the `sha256` from the PyPI sdist:
   `pip download --no-binary :all: --no-deps scix-client && shasum -a 256 scix-client-*.tar.gz`.
4. Open a PR. Once merged, conda-forge generates a feedstock repo that
   auto-rebuilds on each PyPI release.

Users install with:

```bash
conda install -c conda-forge scix-client
```

## Docker / OCI (`../Dockerfile`)

Built and pushed to `ghcr.io/yipihey/scix-client` by the `docker` job in
`.github/workflows/release.yml`. No external setup needed beyond the default
`GITHUB_TOKEN` (already wired).

Users run with:

```bash
docker run --rm -e SCIX_API_TOKEN=... ghcr.io/yipihey/scix-client search "dark matter"
```
