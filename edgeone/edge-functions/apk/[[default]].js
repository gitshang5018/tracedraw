// TraceDraw's update host: `/apk/<file name>` -> the APK of a release.
//
// The Android workflow uploads each release's APKs into the blob store
// under `apk/<file name>` (see `scripts/upload-edgeone.mjs`), and this
// function is where the app's update check downloads them from. Nothing
// else is served.
//
// Blob is meant for a project's own data rather than as a public file
// host, so the only names this answers for are the ones the release
// workflow gives its files.
import { getStore } from "@edgeone/pages-blob";

const STORE = "tracedraw";
/// tracedraw-<version>-android-<abi>.apk
const APK = /^tracedraw-[0-9A-Za-z][0-9A-Za-z.\-]*-android-[0-9A-Za-z_\-]+\.apk$/;

async function onRequest({ request }) {
  if (request.method !== "GET") {
    return new Response("Method not allowed", { status: 405 });
  }
  const name = new URL(request.url).pathname.split("/").pop() ?? "";
  if (!APK.test(name)) {
    return new Response("Not found", { status: 404 });
  }

  const store = getStore(STORE);
  const file = await store.get(`apk/${name}`, { type: "arrayBuffer" });
  if (!file) {
    return new Response("Not found", { status: 404 });
  }
  return new Response(file, {
    headers: {
      "Content-Type": "application/vnd.android.package-archive",
      "Content-Length": String(file.byteLength),
      "Content-Disposition": `attachment; filename="${name}"`,
      // The app asks once per release; a short cache keeps a retry cheap
      // without holding a replaced file for long.
      "Cache-Control": "public, max-age=300",
    },
  });
}

// The runtime accepts either shape of handler, and the two documents show
// one each.
export default onRequest;
export { onRequest };
