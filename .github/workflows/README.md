# Full-Service-Mirror Build Release Process

This Github Action Workflow provides an automated build release process for the full-service-mirror and associated binaries. There are currently two workflows provided.

- ### _tag.yaml_ - Creates a new TAG depending on which branch your code is pushed to.

  - `release/v*` branch will create a new prerelease tag in the format `vx.x.x-pre.x` e.g - v1.5.1-pre.1
  - `main` branch will create a full release tag in the format `vx.x.x` e.g. - `v1.5.1`

  <br/>

  > **NOTE: By default the tag is bumped at the patch level so if the current release is at `v1.5.0` then a new tag will be created as `v1.5.1-pre.1` when a release/\* branch is pushed. If this release branch is then merged into the main branch a new release tag will be created as `v1.5.1`**

  > The bump level can be overridden by supplying the following on the commit message **#major, #minor, #none**

- ### _build.yaml_ - Builds and uploads artifacts and release binaries. Also builds and pushes docker files to docker hub
  - Runs when code is pushed to the following branches or any tag:
    - develop
    - feature/\*
    - hotfix/\*

## Push code to repo for any of the following branches

- `develop`
- `feature/*`
- `hotfix/*`

1. Code is built
1. Artifact is created and uploaded to workflow output

## Push code to repo for the following branches

- `release/*`
- `main`

1. Tag workflow is run creating a new tag for the release.
1. Build workflow is run because of the `tag` push
1. Code is built
1. Artifact is created and uploaded to the workflow output
1. `Release branch`
   1. A new prerelease is created
   1. The artifacts are uploaded to the github releases for the current tag
1. `Main branch`
   1. Latest prerelase artifact is downloaded (Promoting the tested binaries)
   1. A new release is created
   1. Artifact is uploaded as an official release
