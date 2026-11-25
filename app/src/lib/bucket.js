// A cloud bucket: files kept with a storage provider (Cloudflare R2, Backblaze B2, any S3),
// shown in Storage beside My Drive and This computer.
//
// The box holds the bucket's keys and does every call (see castled/src/bucket.rs); the app
// only ever talks to the box. An item there has an id of "bucket:" followed by its whole
// name in the bucket ("bucket:music/2026/song.mp3"). A folder's name ends with a slash, and
// the top of the bucket is "bucket:" alone.

export const BUCKET_PREFIX = 'bucket:';
/** The top of the bucket. */
export const BUCKET_ROOT = BUCKET_PREFIX;
export const BUCKET_NAME = 'Bucket';

export const isBucketId = (id) => typeof id === 'string' && id.startsWith(BUCKET_PREFIX);
export const bucketKey = (id) => (isBucketId(id) ? id.slice(BUCKET_PREFIX.length) : '');
export const bucketId = (key) => BUCKET_PREFIX + key;

/** The breadcrumbs from the top of the bucket down to the folder `key` ("music/2026/"). */
export function bucketTrail(key = '', name = BUCKET_NAME) {
  const trail = [{ id: BUCKET_ROOT, name }];
  let at = '';
  for (const part of key.split('/').filter(Boolean)) {
    at += `${part}/`;
    trail.push({ id: bucketId(at), name: part });
  }
  return trail;
}

/** "Gta6 Freestyle!" -> "gta6-freestyle": a name safe in an address and as an id. */
export function slug(text) {
  return String(text || '')
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}
