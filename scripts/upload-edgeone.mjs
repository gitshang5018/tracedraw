// Put a release's APKs into EdgeOne Makers' blob storage, where the app's
// update check downloads them from (`https://<project>/apk/<file name>`).
//
// Run by the Android workflow after the APKs are built:
//
//   node scripts/upload-edgeone.mjs artifacts
//
// with EDGEONE_PROJECT_ID and EDGEONE_TOKEN in the environment. Without
// them nothing is uploaded and the step is a no-op, so a repository that
// has not set them up still publishes its releases.
//
// Only the files of this run are kept: the update check always offers the
// newest build, so an older APK is of no use to anyone and the free plan's
// 1 GB would fill up with a few weeks of nightlies.
import { getStore } from "@edgeone/pages-blob";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

const STORE = "tracedraw";
const PREFIX = "apk/";

const dir = process.argv[2] ?? "artifacts";
const projectId = process.env.EDGEONE_PROJECT_ID;
const token = process.env.EDGEONE_TOKEN;

if (!projectId || !token) {
  console.log(
    "EDGEONE_PROJECT_ID and EDGEONE_TOKEN are not set: the APKs stay on the release only.",
  );
  process.exit(0);
}

const files = (await readdir(dir)).filter((name) => name.endsWith(".apk"));
if (files.length === 0) {
  console.error(`No APK in ${dir}.`);
  process.exit(1);
}

const store = getStore({ name: STORE, projectId, token });
const kept = new Set();

for (const name of files) {
  const bytes = await readFile(join(dir, name));
  await store.set(`${PREFIX}${name}`, new Blob([bytes]));
  kept.add(`${PREFIX}${name}`);
  console.log(`uploaded ${PREFIX}${name} (${bytes.length} bytes)`);
}

// The files of the build before go: the newest build is the one the check
// offers, and the store is small.
const { blobs } = await store.list({ prefix: PREFIX, consistency: "strong" });
for (const { key } of blobs) {
  if (!kept.has(key)) {
    await store.delete(key);
    console.log(`removed ${key}`);
  }
}
