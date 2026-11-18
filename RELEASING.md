# Beatnik Hardware API: Tagging & Releasing

Releases are built by GitHub Actions whenever a tag like `v0.9.0` is pushed. Locally you only bump the version, commit, tag, and push. [cargo-release](https://github.com/crate-ci/cargo-release) does the first three in one command, like `npm version` did.

## One-time setup

```bash
cargo install --locked cargo-release
```

The settings live in [`release.toml`](release.toml). They make `cargo release` behave like `npm version`:

| Setting | Effect |
|---|---|
| `pre-release-commit-message = "{{version}}"` | Commit is named after the version (e.g. `0.9.1`) |
| `tag-name = "v{{version}}"` | Tag `v0.9.1`, which triggers the release workflow |
| `push = false` | Nothing is pushed; you push manually |
| `publish = false` | Nothing is published to crates.io |
| `allow-branch = ["master"]` | Releases can only be cut from `master` |

## Release in short

```bash
git checkout master && git pull
cargo release patch --execute   # like: npm version patch
git push && git push --tags
```

## Step by step

### 1. Prepare `master`

```bash
git checkout master
git pull
cargo test
```

The working tree must be clean; cargo-release refuses to run with uncommitted changes.

### 2. Bump, commit and tag

| npm | cargo-release | Example |
|---|---|---|
| `npm version patch` | `cargo release patch --execute` | 0.9.0 → 0.9.1 |
| `npm version minor` | `cargo release minor --execute` | 0.9.1 → 0.10.0 |
| `npm version major` | `cargo release major --execute` | 0.10.0 → 1.0.0 |
| `npm version 1.2.3` | `cargo release 1.2.3 --execute` | → 1.2.3 |

Without `--execute`, cargo-release only does a **dry run** and shows what it would do. That's useful for checking first:

```bash
cargo release patch
```

With `--execute` it asks for confirmation, then:
1. sets `version` in `Cargo.toml` and updates `Cargo.lock`,
2. commits both as `<version>`,
3. creates the annotated tag `v<version>`.

Check the result:

```bash
git log --oneline -1   # e.g. "a1b2c3d 0.9.1"
git tag --list 'v*' --sort=-v:refname | head -3
```

### 3. Push

```bash
git push && git push --tags
```

Pushing the tag starts the [Build and Release](.github/workflows/release.yml) workflow. It:
1. checks that the tag matches the version in `Cargo.toml`, and runs the tests,
2. cross-compiles for `arm64` and `armhf`,
3. creates the GitHub release with these assets:
   - `beatnik-hardware-api-arm64.tar.gz`, `beatnik-hardware-api-armhf.tar.gz` (for `setup.sh`)
   - `beatnik-hardware-api_<version>_arm64.deb`, `beatnik-hardware-api_<version>_armhf.deb`

Follow it under the repository's **Actions** tab. Expect a few minutes for the build.

### 4. Verify

- The release appears under [Releases](https://github.com/byrdsandbytes/beatnik-hardware-api/releases) with all four assets.
- On a Pi, `POST /api/update/api` (package install) or `./setup.sh` (script install) picks up the new version. `GET /api/update/versions` shows the installed version.

## Without cargo-release

The same steps by hand:

```bash
# 1. Edit the version in Cargo.toml, e.g. version = "0.9.1"
cargo check                       # updates Cargo.lock
git commit -am "0.9.1"
git tag -a v0.9.1 -m "0.9.1"
git push && git push --tags
```

## Troubleshooting

**`cannot release from branch ... as it doesn't match master`**
Switch to `master`. To release from another branch once, add `--allow-branch <branch>`.

**`uncommitted changes detected`**
Commit or stash your changes first.

**Workflow fails at "Check tag matches Cargo.toml version"**
The tag was created without bumping `Cargo.toml` (e.g. a manual `git tag`). Delete the tag locally and on GitHub, then tag the right commit:
```bash
git tag -d v0.9.1
git push --delete origin v0.9.1
```

**A release failed halfway, or a broken release went out**
Delete the GitHub release (if it was created) and the tag as above. Fix the problem, then release the **next** version. Devices may already have seen the broken tag, and the in-app updater only installs when the version differs from the installed one.

**Re-running a failed workflow**
If the failure was temporary (e.g. a network error), use **Re-run jobs** on the workflow run in the Actions tab. No new tag is needed.
