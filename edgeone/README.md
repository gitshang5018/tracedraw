# TraceDraw's update host

A release's APKs are published as files of this site, so a phone downloads
its update from Tencent's edge network instead of from GitHub, which a phone
behind a filtered network cannot reach at all.

The app still asks GitHub which build is the newest (`updates.rs`), and
checks the SHA-256 digest GitHub records for the file; only the file itself
comes from here. When the host has not got a release's APK, the app falls
back to the release's own address, so a missing deployment costs speed
rather than the update.

## Why files and not a function

An Edge Function answers within 200 ms of CPU, and a Cloud Function may
return a response of 6 MB at most, while an APK is 23 to 26 MB: a function
cannot hand one over. A static file is served by the CDN with no such limit,
beyond the 25 MiB a single page of a site may have.

The x86_64 APK (for emulators) is 25.6 MiB, over that: the workflow leaves
it on the release, where the app still finds it. The two phone ABIs fit.

## What is here

```text
edgeone/
  index.html      a page that says what this is
  package.json    the project's manifest, so the CLI takes the folder
```

The workflow makes the site to deploy: `index.html`, `package.json` and
`apk/<file name>` for every APK under 25 MiB. `/apk/<file name>` is what the
app asks for.

## Making the project

1. Open [pages.edgeone.ai](https://pages.edgeone.ai) and sign in.
2. On the console's home page, **创建项目**, and choose **直接上传** (direct
   upload): the workflow deploys to it with the CLI, and the CLI wants a
   project of that kind.
3. Give it a name (5 to 50 characters, lower-case letters, digits and
   hyphens; the workflow expects `tracedraw`), and choose the acceleration
   region.
4. Drag this folder's contents in, and **开始部署**. The deployment's page
   shows the address the project answers on, like
   `https://tracedraw.edgeone.dev`.

## Wiring it to the app

1. **The API token** is a tab of the Makers console itself, beside the
   project list (API Token > 创建 API Token > a description, an expiry date,
   提交). It is not per project.
2. Add it to the GitHub repository, Settings > Secrets and variables >
   Actions, as `EDGEONE_TOKEN`. Without it the workflow deploys nothing and
   the app downloads from the release, exactly as it did before.
3. Put the project's address in `apps/tracedraw/src/updates.rs` as
   `APK_HOST`.
4. Push to `main`: the workflow builds the APKs and deploys them as the
   site's own files.

## Checking it by hand

```sh
curl -I https://tracedraw.edgeone.dev/apk/tracedraw-0.19.9-dev.d41c92a-android-arm64-v8a.apk
```

answers `200` with the file's size; a name that is not there answers `404`.

A function under `edge-functions/` cannot serve these files: it answers in
200 ms of CPU and the platform returns `545` when a script cannot finish in
it. That is why the APKs are pages of the site.
