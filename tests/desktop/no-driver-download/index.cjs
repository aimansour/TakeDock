// The test suite requires external, version-matched drivers. No archive is extracted.
// Remove this guard when the unused manager no longer depends on vulnerable extract-zip.
module.exports = async function () {
  throw new Error(
    'Automatic browser/driver downloads are disabled. Install the pinned external drivers using scripts/setup-desktop-drivers.ps1.',
  );
};
