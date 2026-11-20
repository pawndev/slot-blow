# slot-blow: fork of slot-cfw/slot

This repository is **pawndev/slot-blow**, a fork of upstream **slot-cfw/slot**.
Everything below is fork-only. An upstream sync must keep all of it; when a merge
conflicts on one of these spots, keep the fork's side and adapt it to upstream's new code.

This file lives in `.claude/CLAUDE.md` on purpose: upstream's `.gitignore` ignores `/CLAUDE.md`.

## Branches

- `main` mirrors upstream `main`. Never commit fork work on it.
- `pawn-main` is the fork: upstream `main` merged in, plus the changes below. Releases are cut from it.
- Sync = update `main` from upstream, then **merge** (never rebase) `main` into `pawn-main`
  and every open PR branch (the `sync-fork-branches` skill does this).
- There is no `upstream` remote; reach upstream with `https://github.com/slot-cfw/slot.git`.

## Fork divergences to preserve

### Gameplay features (Quick menu → Gameplay → Experimental)

Shown in the README's "Added in this fork" section (`README.md`, `.github/resources/screenshots/fork/`).

- **Dusty Carts**: one cart in ten refuses to boot until you blow on it (hold B).
  `crates/slot/src/dust.rs`, tests `crates/slot/tests/dust.rs`, `render_dust.rs`.
- **Label Wear**: cart labels wear down with play time.
  `crates/slot-ui/src/wear.rs`, play time in `crates/slot-store/src/play_time.rs`,
  tests `crates/slot-ui/tests/{wear,render_wear}.rs`, `crates/slot-store/tests/play_time.rs`, `crates/slot/tests/play_time.rs`.
- **Dry Batteries**: a cart's save battery runs dry and you solder in a new one.
  `crates/slot/src/dry.rs`, `crates/slot/src/pixel_font.rs`, tests `crates/slot/tests/dry.rs`.
- Hooks into shared upstream files, the usual conflict spots: `crates/slot/src/app.rs`
  (`QuickRow::Experimental`), `frontend.rs`, `cart_faces.rs`, `face_builder.rs`, `session.rs`,
  `audio/sfx.rs`, `host_app.rs`, `device_app.rs`, `lib.rs`; `crates/slot-ui/src/{cart,quick_menu,lib}.rs`;
  `crates/slot-store/src/{slot_state,lib}.rs`.
- To see the full current divergence: `git diff --stat main...pawn-main`.

### OTA updater points at the fork

- `crates/slot/src/update.rs`: `LATEST` and `RELEASES` use
  `github.com/pawndev/slot-blow/releases/...`, not `slot-cfw.fyi` / `slot-cfw/slot`
  (`crates/slot/tests/update.rs` asserts these URLs). `BASEOS_RELEASES` stays on `pvaibhav/BaseOS`.
- `.github/workflows/release.yml`: the publish job attaches `versions.json` to the release
  (built by `.github/scripts/versions.sh` with `SLOT_REPO=$GITHUB_REPOSITORY`) instead of
  triggering `pages.yml`.
- `.github/workflows/versions.yml`: fork-only, refreshes `versions.json` on the latest release twice a day.

### Test fixes upstream does not have

- `crates/slot/tests/render_studio.rs`: `for_a_while()` waits up to 20s of wall clock for the studio's
  background-built faces in `a_label_that_cannot_be_read_stays_on_the_sheet`,
  `a_label_that_will_not_decode_counts_as_handwriting` and `apply_run`. Upstream's fixed frame
  counts (~1s) fail under load and block `task tag`. Upstream CI (`lint.yml`) never runs tests,
  so upstream may break more of these; drop this once upstream fixes it.
  The same fix, based on upstream `main`, is PR #6 (`fix/studio-test-waits`, never merged into
  the fork's `main`); its `.patch` is what to hand upstream when it takes contributions again.

### Release DAG (`taskfile.yml`)

- Global vars `FORK_BRANCH: pawn-main` and `UPSTREAM_REPO: slot-cfw/slot`.
- `release:fork` → `release:fork:tag` → `release:fork:check` (`:branch`, `:upstream`, `:changelog`, `:free`).

## Releasing

Fork releases reuse the upstream version number they are built on.

1. Sync `main` from upstream and merge it into `pawn-main`, push.
2. `task release:fork:check VERSION=vX.Y.Z`: checks that HEAD is a clean, pushed `pawn-main` that
   contains upstream's `vX.Y.Z` tag, that `CHANGELOG.md` has a `## [X.Y.Z]` entry, and that the tag is free.
3. `task release:fork VERSION=vX.Y.Z`: runs `task tag` (lints and tests, then the annotated
   `Checked: <sha>` tag that `release.yml` requires), pushes the tag, watches the workflow and checks
   that the release has `slot-vX.Y.Z-h700.zip` and `versions.json`.

Never fetch upstream's tags into this clone: a local `vX.Y.Z` pointing at upstream's commit blocks `task tag`.
Release notes come from the upstream `CHANGELOG.md` entry, which does not mention the fork features.
