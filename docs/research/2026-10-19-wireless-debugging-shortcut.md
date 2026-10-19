# Native phone shortcut for wireless debugging

The user requested a phone shortcut for toggling wireless debugging and reaching
its settings without repeatedly navigating Developer options.

On the authorized Samsung SM-A155F running Android 16, the installed Settings
package contains the native
`com.android.settings.development.qstile.DevelopmentTiles$WirelessDebugging`
service. Its component was disabled and its tile was absent from Quick Settings.
Samsung rejects shell `pm enable` for this protected Settings component with a
SecurityException. That unsuccessful command did not change its state.

The supported Settings UI was used: Developer options → Quick settings developer
tiles → Wireless debugging. This enabled the native component. The existing
`cmd statusbar add-tile`/`set-tiles` interface then added its tile in the first
position, preserving all 18 previous tile specs and their relative order. There
are now 19. Screenshot inspection confirmed the tile is visible at the top, and
an actual long press opened the Wireless debugging page directly.

The existing `adb_wifi_enabled` value and enabled TalkBack were preserved.
No pairing was changed. The on/off tile action was not exercised during this
configuration because its existing debug connection was retained. Raw screenshots
and the original tile-list backup remain private under ignored `test-results/`.
No device addresses, pairing codes or screen credentials belong in repository
documentation or tests.

For the later in-app ADB setup design, do not assume shell component enabling is
available on every phone. The verified native UI route is a useful fallback. An
already available phone tile continues to provide local access when the PC has
no ADB transport. This configuration did not add a new product command or APK.

The native service is also present in the
[AOSP Android 16 Settings manifest](https://android.googlesource.com/platform/packages/apps/Settings/+/android16-qpr1-release/AndroidManifest.xml).
The conclusions above about this Samsung device come from its installed package
and the observed phone UI.
