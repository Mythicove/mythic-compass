# grasscn registry components

Button, Input, Progress and their supporting files are copied from the
[grasscn registry](https://grasscn.dev/docs), maintained at
[nicognaW/grasscn](https://github.com/nicognaW/grasscn), reviewed 2026-11-10.
The upstream MIT license is retained in `GRASSCN-LICENSE.txt`; assets keep their
upstream in-file provenance. Registry URLs: `/r/button.json`, `/r/input.json`,
`/r/progress.json`. This is source vendoring, not a dependency on the demo site.

The launch page imports these components directly. Controls use local embedded
artwork, click sound and Unicode glyph data; there is no runtime texture CDN.
The interface sound checkbox uses the registry's existing volume API. The
legacy page CSS is in a lower cascade layer so it cannot repaint MC controls.

Only the launch/sign-in workflow is migrated in this stage. Existing instance,
content, settings and logs pages retain their own controls pending migration.
The existing skinview3d preview remains, since grasscn's alternate player
renderer has different skin/WebGL constraints.

Measured local Vite build: approximately 1.58 MB CSS, 1.07 MB main JavaScript,
and 2.08 MB lazy Unicode data before compression. This is an intentional offline
asset tradeoff, not a claim that the registry has zero bundle cost. Chromium
tests exercise long Chinese labels, password entry, launch/retry/stop, scale
1.25/1.5 and 960×640 / 1200×800. A real Windows Tauri/WebView2 build was also
opened from a fresh portable directory and verified for Chinese input, offline
assets, disabled launch without an account, and no horizontal overflow. Native
macOS/Linux and real-game launch from the new page remain separate acceptance
items; browser launch/retry/stop uses the external IPC transport fixture.
