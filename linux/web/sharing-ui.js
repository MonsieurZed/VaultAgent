/* L'edition charge seulement des metadonnees ; les secrets existants restent au serveur. */
export function initSharing({ api, action, t }) {
  const $ = id => document.getElementById(id);
  let info = []; let items = []; let editing = null; let plan = null; let operationId = crypto.randomUUID();
  const option = (value, label) => { const node = document.createElement('option'); node.value = value; node.textContent = label; return node; };
  const clearSecrets = () => { for (const field of ['username', 'password', 'totp', 'notes']) $('shared-' + field).value = ''; };
  function itemType() { const login = $('shared-type').value === '1'; for (const field of ['username', 'password', 'totp', 'uri']) $('shared-' + field).closest('label').hidden = !login; }
  function newItem() {
    editing = null; operationId = crypto.randomUUID(); $('shared-form').reset();
    $('share-org').disabled = $('share-collection').disabled = $('shared-type').disabled = $('shared-uri').disabled = false;
    itemType();
    render();
  }
  function clearPlan() { plan = null; $('share-confirm').checked = false; $('share-apply').disabled = true; $('share-summary').textContent = ''; }
  function collections() {
    $('share-collection').replaceChildren(); const org = info.find(org => org.id === $('share-org').value);
    for (const collection of org?.collections || []) $('share-collection').append(option(collection.id, collection.name + (collection.canEdit ? '' : ' · ' + t('Lecture seule'))));
    clearPlan(); render();
  }
  function render() {
    $('shared-items').replaceChildren();
    const collectionId = $('share-collection').value; const org = info.find(org => org.id === $('share-org').value);
    $('sharing-hint').hidden = info.length > 0;
    $('collection-save').disabled = !org;
    $('shared-save').disabled = !org || !(editing ? org.collections.some(collection => editing.collectionIds.includes(collection.id) && collection.canEdit) : org.collections.some(collection => collection.id === collectionId && collection.canEdit));
    for (const item of items.filter(item => item.organizationId === org?.id && item.collectionIds.includes(collectionId))) {
      const row = document.createElement('div'); row.className = 'device'; const label = document.createElement('strong'); label.textContent = item.name;
      const edit = document.createElement('button'); edit.type = 'button'; edit.className = 'quiet'; edit.textContent = t('Modifier');
      edit.disabled = !org.collections.some(collection => item.collectionIds.includes(collection.id) && collection.canEdit);
      edit.addEventListener('click', () => {
        newItem(); editing = item; $('shared-name').value = item.name; $('shared-type').value = item.type;
        $('share-org').disabled = $('share-collection').disabled = $('shared-type').disabled = $('shared-uri').disabled = true;
        itemType();
        $('shared-save').disabled = false;
        $('shared-name').focus();
      }); row.append(label, edit); $('shared-items').append(row);
    }
  }
  async function load() {
    newItem();
    const previousOrg = $('share-org').value; const previousCollection = $('share-collection').value;
    const [sharing, catalog] = await Promise.all([api('sharing', {}), api('write-preview', {})]);
    info = sharing.organizations; items = catalog.items;
    $('share-org').replaceChildren(); for (const org of info) $('share-org').append(option(org.id, org.name));
    if (info.some(org => org.id === previousOrg)) $('share-org').value = previousOrg;
    collections();
    if ([...$('share-collection').options].some(option => option.value === previousCollection)) $('share-collection').value = previousCollection;
    $('personal-item').replaceChildren(); for (const item of items.filter(item => !item.organizationId)) $('personal-item').append(option(item.id, item.name));
    render();
  }
  $('sharing-refresh').addEventListener('click', () => action($('sharing-refresh'), load));
  $('share-org').addEventListener('change', collections); $('share-collection').addEventListener('change', () => { clearPlan(); render(); });
  $('shared-new').addEventListener('click', newItem);
  $('shared-type').addEventListener('change', itemType);
  $('collection-form').addEventListener('submit', event => { event.preventDefault(); action(event.submitter, async () => {
    await api('share', { action: 'create-collection', operationId: crypto.randomUUID(), organizationId: $('share-org').value, name: $('collection-name').value });
    $('collection-name').value = ''; await load();
  }); });
  $('shared-form').addEventListener('submit', event => { event.preventDefault(); action(event.submitter, async () => {
    const fields = {}; for (const field of ['username', 'password', 'totp', 'notes']) if ($('shared-' + field).value) fields[field] = $('shared-' + field).value;
    let request;
    if (editing) request = { action: 'update', operationId, itemId: editing.id, expectedRevisionDate: editing.revisionDate, organizationId: editing.organizationId, patch: { name: $('shared-name').value, ...fields } };
    else {
      const type = Number($('shared-type').value); const { notes, ...login } = fields;
      request = { action: 'create', operationId, item: { type, name: $('shared-name').value, organizationId: $('share-org').value, collectionIds: [$('share-collection').value],
        ...(notes === undefined ? {} : { notes }), ...(type === 1 ? { login: { ...login, uris: $('shared-uri').value ? [{ uri: $('shared-uri').value }] : [] } } : {}) } };
    }
    try { await api('write', request); newItem(); await load(); }
    finally { clearSecrets(); }
  }); });
  $('share-preview').addEventListener('click', () => action($('share-preview'), async () => {
    clearPlan(); const input = { action: 'preview', itemId: $('personal-item').value, organizationId: $('share-org').value, collectionIds: [$('share-collection').value] };
    const preview = await api('share', input); plan = { ...input, action: 'apply', operationId: crypto.randomUUID(), expectedRevisionDate: preview.expectedRevisionDate };
    $('share-summary').textContent = preview.name + ' → ' + preview.organizationName;
  }));
  $('share-confirm').addEventListener('change', () => $('share-apply').disabled = !plan || !$('share-confirm').checked);
  $('share-apply').addEventListener('click', () => action($('share-apply'), async () => {
    if (!plan || !$('share-confirm').checked) throw new Error('Opération refusée.');
    await api('share', { ...plan, confirmOwnershipTransfer: true }); clearPlan(); await load();
  }));
  return { localize: render, reset() { newItem(); clearPlan(); $('collection-name').value = ''; info = []; items = []; for (const id of ['share-org', 'share-collection', 'personal-item', 'shared-items']) $(id).replaceChildren(); render(); } };
}
