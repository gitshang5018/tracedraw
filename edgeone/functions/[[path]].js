// TraceDraw's update host.
//
// The Android workflow uploads each release's APKs into the blob store
// under `apk/<file name>` (see `scripts/upload-edgeone.mjs`), and this
// function serves them at `/apk/<file name>`, which is where the app's
// update check downloads from. Nothing else is served.
//
// Blob is meant for a project's own data rather than as a public file
// host, so the only keys this answers for are the names the release
// workflow gives its files.
import { getStore } from "@edgeone/pages-blob";

const STORE = "tracedraw";
/// tracedraw-<version>-android-<abi>.apk
const APK = /^tracedraw-[0-9A-Za-z][0-9A-Za-z.\-]*-android-[0-9A-Za-z_\-]+\.apk$/;

export async function onRequest({ request }) {
  const url = new URL(request.url);
  const path = decodeURIComponent(url.pathname).replace(/^\/+/, "");

  if (request.method !== "GET") {
    return new Response("Method not allowed", { status: 405 });
  }
  if (path === "" || path === "index.html") {
    return new Response("TraceDraw updates. The app downloads from /apk/.\n", {
      headers: { "Content-Type": "text/plain; charset=utf-8" },
    });
  }
  const name = path.startsWith("apk/") ? path.slice("apk/".length) : "";
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
