/* Modifier le CipherView en place conserve les passkeys et les champs non ciblés. */
export function applyWritePatch(view, expectedRevision, patch) {
  const allowed = ['name', 'folderId', 'notes', 'username', 'password', 'totp'];
  if (!patch || typeof patch !== 'object' || Array.isArray(patch) || !Object.keys(patch).length || Object.keys(patch).some(k => !allowed.includes(k))) throw new Error('invalid-patch');
  if (!expectedRevision || new Date(view.revisionDate).toISOString() !== new Date(expectedRevision).toISOString()) throw new Error('revision-conflict');
  if (view.organizationId || view.isDeleted || view.isArchived || view.deletedDate || view.archivedDate || ![1, 2].includes(view.type)) throw new Error('item-refused');
  for (const [key, value] of Object.entries(patch)) {
    if (key === 'folderId') { if (value !== null && !/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(value)) throw new Error('invalid-patch'); }
    else if (typeof value !== 'string' || value.length > (key === 'notes' ? 12000 : 8192) || key === 'name' && (!value.trim() || value.length > 500 || /[\x00-\x1f]/.test(value))) throw new Error('invalid-patch');
    if (['username', 'password', 'totp'].includes(key)) {
      if (view.type !== 1 || !view.login) throw new Error('item-refused');
      view.login[key] = value;
    } else view[key] = value;
  }
  return view;
}
