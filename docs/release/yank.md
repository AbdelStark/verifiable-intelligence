# Release Yank Procedure

Use this procedure when a published release fails a documented release gate after
publication. Yanking means "stop recommending this release and publish a fixed
patch"; it does not mean deleting immutable evidence.

## When To Yank

Yank a release when one of these is true:

- a proof bundle, verifier, or schema gate accepts an artifact it should reject,
- a published binary, WASM package, or provider image does not match the release
  notes or checksums,
- a hosted demo misrepresents simulated fixtures as live provider traffic,
- the release violates the lawful-use or proof-boundary language in public docs,
- a security issue requires users to stop using the release.

Do not yank for ordinary documentation typos unless they create a misleading
proof or security boundary.

## Inputs

Set these values before running commands:

```bash
export VERSION="1.0.0"
export TAG="v${VERSION}"
export NEXT_PATCH="1.0.1"
export YANK_REASON="brief-reason-slug"
export CRATE="verifiable-intelligence"
export IMAGE="ghcr.io/abdelstark/verifiable-intelligence-provider"
export IMAGE_DIGEST="sha256:<published-image-digest>"
```

For a static demo prerelease, use the release tag, for example:

```bash
export TAG="demo-v0.1.0"
export YANK_REASON="fixture-boundary-copy"
```

## 1. Freeze The Release

1. Stop ongoing announcement or deployment work.
2. Open or update a GitHub issue titled `release: yank ${TAG}`.
3. Record:
   - release URL,
   - failing gate,
   - first bad commit or tag,
   - user-visible impact,
   - mitigation users should take,
   - owner for the patch release.

## 2. Yank The Crate

Only run this if the crate version was published to crates.io.

```bash
cargo yank --crate "$CRATE" --vers "$VERSION"
```

If a later investigation proves the yank was unnecessary, undo it with:

```bash
cargo yank --crate "$CRATE" --vers "$VERSION" --undo
```

Record the cargo command output in the yank issue.

## 3. Mark The Provider Image

Do not delete or mutate the original image tag. Preserve immutability and add a
clear yanked marker tag pointing at the same digest:

```bash
docker pull "${IMAGE}@${IMAGE_DIGEST}"
docker tag "${IMAGE}@${IMAGE_DIGEST}" "${IMAGE}:yanked-${VERSION}-${YANK_REASON}"
docker push "${IMAGE}:yanked-${VERSION}-${YANK_REASON}"
```

If the semver tag points at a bad digest and your registry policy allows moving
tags, move only mutable convenience tags such as `latest` or `edge` to a fixed
release. Do not rewrite the digest evidence in release notes.

## 4. Mark The GitHub Release

Keep the release visible. Edit the title and notes so users see the warning
before downloading artifacts:

```bash
gh release edit "$TAG" \
  --prerelease \
  --title "YANKED: ${TAG}" \
  --notes-file /tmp/yanked-${TAG}.md
```

Prepare `/tmp/yanked-${TAG}.md` with:

```markdown
# YANKED: <tag>

This release is yanked because <reason>.

Impact: <who is affected and how>.

Mitigation: use <fixed version> or stay on <known-good version>.

Evidence: <issue or advisory URL>.
```

If the release was a public stable release, also create or update a GitHub
Security Advisory when the reason is security-sensitive.

## 5. Patch Release

Create a patch branch from `main` unless the fix must be backported:

```bash
git switch main
git pull --ff-only
git switch -c "release/v${NEXT_PATCH}"
```

Patch the issue, then run the release gates that failed plus the broad gates:

```bash
npm run test:bundle
npm run test:demo
cargo test --workspace --all-features --locked
git diff --check
```

For provider-image or binary-release failures, also run the matching release
workflow or documented manual replacement once `release.yml` exists.

Update `CHANGELOG.md` with:

- the yanked version and reason,
- the patch version and fix,
- any user migration instruction.

Tag and publish the patch release only after the failed gate is green and the
yank issue links to the passing evidence.

## 6. Close The Loop

Before closing the yank issue:

- [ ] Crate yanked or confirmed unpublished.
- [ ] Provider image marked with `yanked-${VERSION}-${YANK_REASON}` or confirmed
      unpublished.
- [ ] GitHub Release title and notes mark the release as yanked.
- [ ] Fixed patch release published, or known-good rollback documented.
- [ ] README, release notes, and CHANGELOG point users at the fixed version.
- [ ] Any hosted demo deployment has been republished or disabled.
