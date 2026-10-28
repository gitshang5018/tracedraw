# TraceDraw's update host

The APKs of a release are kept in EdgeOne Makers' blob storage and served
by one edge function, so a phone downloads its update from Tencent's edge
network instead of from GitHub, which a phone behind a filtered network
cannot reach at all.

The app still asks GitHub which build is the newest (`updates.rs`), and
checks the SHA-256 digest GitHub records for the file; only the file itself
comes from here. When the storage has not got a release's APK yet, the app
falls back to the release's own address, so a missing upload costs speed
rather than the update.

## What is here

```text
edgeone/
  index.html                        a page that says what this is
  package.json                      the function's dependency
  edge-functions/
    apk/[[default]].js              GET /apk/<file name> -> the file from the store
```

`GET /apk/tracedraw-0.19.9-dev.4c864d1-android-arm64-v8a.apk` answers with
the APK, `application/vnd.android.package-archive`. Nothing else is served:
the store is a project's own data, not a file host.

## Making the project

1. Open [pages.edgeone.ai](https://pages.edgeone.ai) and sign in.
2. On the console's home page, **创建项目**, and choose **直接上传** (direct
   upload). It runs no build, which is what this folder wants: the function
   is the whole site, and the APKs arrive in the store from the workflow
   rather than with the code.
3. Give it a name (5 to 50 characters, lower-case letters, digits and
   hyphens) and choose the acceleration region.
4. Drag the **contents of this folder** in: `index.html`, `package.json`
   and `edge-functions/` have to sit at the root of what is uploaded, not
   inside another folder.
5. **开始部署**. The deployment's page shows the address the project answers
   on, like `https://<name>.edgeone.app`.

The blob store makes itself: the first request that reads or writes the
`tracedraw` namespace creates it, and the console's Blob page then lists it.

## Wiring it to the app

Two things are needed, and neither of them is in the project's own settings:

1. **The API token** is a tab of the Makers console itself, beside the
   project list (API Token > 创建 API Token > a description, an expiry date,
   提交). It is not per project.
2. **The project id** is the last part of the project's address in the
   browser, `https://pages.edgeone.ai/project/pages-xxxxxxxx`, the
   `pages-xxxxxxxx` bit. `edgeone makers link` in the CLI writes the same id
   into the project's local configuration, for when the address does not
   show it.

Then:
3. Add both to the GitHub repository, Settings > Secrets and variables >
   Actions: `EDGEONE_PROJECT_ID` and `EDGEONE_TOKEN`. Without them the
   Android workflow skips the upload and the app downloads from the release,
   exactly as it did before.
4. Put the project's address in `apps/tracedraw/src/updates.rs` as
   `APK_HOST`.
5. Push to `main`: the workflow builds the APKs, uploads them under `apk/`,
   and drops the ones of the build before.

The store keeps only the newest build's files, since the update check always
offers the newest build and the free plan's 1 GB would otherwise fill up
with nightlies.

A mainland acceleration region answers on a domain that has been filed
(备案); the project's own `.edgeone.app` address needs no filing and answers
from the region the project was made in.

## Checking it by hand

```sh
curl -I https://<name>.edgeone.app/apk/tracedraw-0.19.9-dev.4c864d1-android-arm64-v8a.apk
```

answers `200` with `Content-Type: application/vnd.android.package-archive`;
a name that is not a release file answers `404`. Before the first upload the
same request answers `404` as well, which is expected.

## A project from the repository instead

Importing the repository works too, with this folder as the project's root
and no build command, but the function would then only change with a commit.
The direct upload above is enough: a release changes the store, not the code.
