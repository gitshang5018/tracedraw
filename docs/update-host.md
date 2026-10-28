# The update host

GitHub cannot be reached from every network a phone is on, and its file
host is slow from many of them, so a phone's APK comes from a bucket of the
project's own: Cloudflare R2, with the S3 API. The Android workflow
(`.github/workflows/android.yml`, the "Update host" step) uploads every
build there, and the app downloads from it first and from the release only
when the bucket has not got the file.

Nothing in the code names the address: `build.rs` bakes the repository
variable `UPDATE_HOST` in as `TRACEDRAW_APK_HOST`, which
`updates::APK_HOST` reads. Without the settings below the step prints a
line and uploads nothing, and every phone downloads from the release as it
did before, so a repository that has not set them still releases.

## What the app expects

With `APK_HOST` set, two addresses are read:

| Address | What it is |
|---|---|
| `{APK_HOST}/apk/{file name}` | one APK, under the name the release gives it |
| `{APK_HOST}/update.json?t={seconds}` | the manifest, asked for with a value that changes each time so a cached copy is never the build before |

The manifest names the newest build and its files:

```json
{
  "version": "0.19.9-dev.90c27db",
  "built": 1793097600,
  "page": "https://github.com/gitshang5018/tracedraw/releases",
  "assets": [
    {"name": "tracedraw-0.19.9-dev.90c27db-android-arm64-v8a.apk", "size": 25477955, "sha256": "8d75bcd7..."}
  ]
}
```

`version` is compared with the build that is running; `page` is where a
reader is sent when there is no file to install (empty: the releases page);
an asset is taken only when its name ends in `-android-<abi>.apk` for this
phone's processor, and it is offered for an install in place only when it
carries a size and a SHA-256 of 64 hex digits. The workflow writes `built`
(seconds since 1970) as well, and `build.rs` bakes the same kind of time
into the app.

The address is fixed at build time. Changing `UPDATE_HOST` has no effect on
a phone until it has installed a build made after the change.

## Setting up the bucket

1. Enable R2 in the Cloudflare dashboard. It asks for a payment method even
   when the free allowance is all that is used. A build is about 80 MB and
   the bucket holds one build at a time, so the 10 GB the free tier gives is
   never approached.
2. Create a bucket, for example `tracedraw`. Leave the location on
   **Automatic**: a bucket in the EU, US or FedRAMP jurisdiction answers
   only on `<account id>.eu.r2.cloudflarestorage.com` and its like, while the
   workflow builds the default endpoint from the account id alone.
3. Make it readable. Two routes:
   - **A custom domain** (Settings > Custom Domains > Add, for example
     `updates.example.com`). The domain must be a zone in the same
     Cloudflare account. This is the one to use: it can be cached and
     protected, and it is meant for traffic.
   - **The public development URL** (Settings > Public Development URL >
     Enable). It answers on `https://pub-<hash>.r2.dev` and is rate limited.
     Cloudflare documents it for development only, so it is a way to try the
     step out, not a way to serve updates.
4. Create a token (R2 > Account Details > API Tokens > Manage > Create
   Account API token) with **Object Read & Write**, limited to that bucket.
   Read alone is not enough: the step deletes what is there before it
   uploads. Copy the Access Key ID and the Secret Access Key; the secret is
   shown once.
5. Note the account id, which the dashboard shows in its address
   (`dash.cloudflare.com/<account id>/...`).

## Repository settings

Settings > Secrets and variables > Actions. The bucket name and the address
are variables rather than secrets, since they are printed in the log.

| Kind | Name | Value |
|---|---|---|
| Secret | `R2_ACCOUNT_ID` | the account id |
| Secret | `R2_ACCESS_KEY_ID` | the token's Access Key ID |
| Secret | `R2_SECRET_ACCESS_KEY` | the token's Secret Access Key |
| Variable | `R2_BUCKET` | the bucket name, for example `tracedraw` |
| Variable | `UPDATE_HOST` | the public address, for example `https://updates.example.com` |

`UPDATE_HOST` is a base address: it carries the scheme and no trailing
slash, since the two addresses above are made by adding to it.

## Checking it

Run the Android workflow (Actions > android > Run workflow, or push to
`main`), then:

```bash
curl -s "https://updates.example.com/update.json" | jq
curl -sI "https://updates.example.com/apk/tracedraw-<version>-android-arm64-v8a.apk"
```

The manifest should name the build of that run, and every `sha256` in it
should match `sha256sum` of the file it names. A file that does not match
its digest is thrown away by the app rather than installed.

The step is `continue-on-error: true`, so a failed upload leaves the
release green. Read the step's log: it prints `ls -l dist/apk` before the
upload and `cat dist/update.json` before it, and both should be there.

## When an upload fails

- **`SignatureDoesNotMatch`, `AccessDenied`, `NoSuchBucket`**: the endpoint
  built from `R2_ACCOUNT_ID` does not answer for that bucket and token. Check
  the id, the bucket name and that the token is limited to this bucket, and
  that the bucket is in the default jurisdiction (see above).
- **A checksum error from a service that is not S3 proper**: a recent AWS CLI
  computes a CRC64NVME checksum on every upload unless told not to. The step
  sets `AWS_REQUEST_CHECKSUM_CALCULATION` and
  `AWS_RESPONSE_CHECKSUM_VALIDATION` to `when_required` for this reason; keep
  them if the commands are changed.
- **The upload works but the address answers 404**: public access is off, or
  the custom domain is not connected yet. A bucket is private until step 3.
- **The address answers 404 for one APK only**: the file name in the manifest
  and the object's key differ. The key is `apk/` plus the file name, with no
  version folder.

## Moving from another host

`APK_HOST` is compiled in, so builds made before a change of host keep
asking the old one. They are not stranded: when the releases on GitHub name
a newer build, the download tries the old host first and then the release's
own address through the proxy (`Package.mirror` in `updates.rs`), so the
update still arrives, only more slowly. A phone takes the new address from
the first build it installs after the change.
