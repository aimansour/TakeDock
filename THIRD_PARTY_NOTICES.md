# Third-party notices

TakeDock's original code, Android Observer, file helper, artwork and documentation
are MIT licensed. Open Camera is a separate GPL application; it is not bundled
and none of its source is incorporated. Android APIs and installed application
resources are used at runtime.

The [inventory](third-party/inventory.json) and [license texts](third-party/licenses.txt)
include locked Windows Rust runtime/build dependencies, the Android file-helper
roots, and the production JavaScript dependency tree. Build-only entries may not
be shipped. The installer includes these
notices alongside its executable. Regenerate with `node scripts/third-party-notices.mjs`
after `npm ci` and fetching locked Cargo dependencies. Supplementary license texts
omitted from published crate archives are preserved from their exact upstream
source revisions. Rust standard-library license texts are also included.

MPL-2.0 dependencies are unmodified; their exact source archives are linked in
the inventory and notices and remain available under MPL-2.0. These licenses
do not change the license of TakeDock's independently authored files; see
[Mozilla's distribution guidance](https://www.mozilla.org/en-US/MPL/2.0/FAQ/#using-and-distributing-software-under-the-license).
Dual-licensed dependencies may be used under their permissive option. AND
expressions retain both applicable notices. No GPL-only library is linked.

Rust's standard library is MIT/Apache-2.0; see its
[upstream license](https://github.com/rust-lang/rust/tree/1.98.1) and source.
Microsoft WebView2 and Android Platform Tools are external runtimes/tools under
their publishers' terms. Platform Tools are not included. The Microsoft bootstrapper
may be downloaded by the installer. NSIS packaging tools use their upstream licenses;
they are development dependencies. Test-only WebDriver tools are not shipped.
