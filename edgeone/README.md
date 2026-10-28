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
  package.json            the function's dependency (@edgeone/pages-blob)
  functions/[[path]].js   GET /apk/<file name> -> the file from the store
```

`GET /apk/tracedraw-0.19.9-dev.4c864d1-android-arm64-v8a.apk` answers with
the APK, `application/vnd.android.package-archive`. Nothing else is served:
the store is a project's own data, not a file host.

## Setting it up

1. At [pages.edgeone.ai](https://pages.edgeone.ai), make a project from
   this repository, with this folder as its root (a project of its own: the
   function is the whole site).
2. Deploy it, and note the project's domain (`https://<name>.edgeone.app`).
3. Put that domain in `apps/tracedraw/src/updates.rs` as `APK_HOST`.
4. In the project's settings, make an API token, and add two secrets to the
   GitHub repository (Settings > Secrets and variables > Actions):
   - `EDGEONE_PROJECT_ID`: the project's id (like `pages-urtsvuwmfvli`)
   - `EDGEONE_TOKEN`: the API token

   Without them the Android workflow skips the upload and the app downloads
   from the release, exactly as it did before.
5. Push to `main`: the workflow builds the APKs, uploads them under
   `apk/`, and drops the ones of the build before.

The store keeps only the newest build's files, since the update check always
offers the newest build and the free plan's 1 GB would otherwise fill up
with nightlies.

## Checking it by hand

```sh
curl -I https://<name>.edgeone.app/apk/tracedraw-0.19.9-dev.4c864d1-android-arm64-v8a.apk
```

answers `200` with `Content-Type: application/vnd.android.package-archive`;
a name that is not a release file answers `404`.
