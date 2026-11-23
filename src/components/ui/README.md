# grasscn registry components

Button, Input, Progress, Checkbox, NativeSelect, Textarea and their supporting files are copied from the
[grasscn registry](https://grasscn.dev/docs), maintained at
[nicognaW/grasscn](https://github.com/nicognaW/grasscn), reviewed 2026-11-10.
The upstream MIT license is retained in `GRASSCN-LICENSE.txt`; assets keep their
upstream in-file provenance. Registry URLs: `/r/button.json`, `/r/input.json`,
`/r/progress.json`, `/r/checkbox.json`, `/r/native-select.json`, `/r/textarea.json`.
The last three were added on 2026-11-23, reusing 15 identical existing support files.
Local adaptation: the shared Unicode GPU painter renders in target-canvas coordinates,
so offscreen labels and fields retain their glyphs in long-page captures. The obsolete
viewport-origin API and all three caller registrations were removed together.
This is source vendoring, not a dependency on the demo site.

All five active pages and the shell import these components directly. Controls use local embedded
artwork, click sound and Unicode glyph data; there is no runtime texture CDN.
The interface sound checkbox uses the registry's existing volume API. The
application stylesheet owns layout dimensions and focus outlines; control artwork,
Unicode painting and interaction come from the registry components.

Launch, instances, content, settings and logs use the same controls.
The existing skinview3d preview remains, since grasscn's alternate player
renderer has different skin/WebGL constraints.

Measured 2026-11-23 Vite build: 1645.77 kB CSS (gzip 528.21 kB),
1127.01 kB main JavaScript (gzip 296.61 kB), and 2079.76 kB lazy Unicode
data (gzip 1509.02 kB). The large-chunk warning remains.

Current acceptance covers all five pages, Chinese/English, dark/light and
1200/760 widths: 40 browser and 40 Windows WebView2 captures. Both browser
tests passed, including mrpack confirmation and offscreen Chinese painting.
The release executable also passed real instance creation and UI-driven saving
of a Chinese name, Unicode JVM textarea, resolution, server and demo setting.
All 40 native matrix entries reported no horizontal overflow. Browser business
scenarios use the external IPC transport fixture. Full online game launch,
Hoard installation and native macOS/Linux were not rerun in this acceptance.
See `UI-REMEDIATION.md` at the repository root for commands and artifact paths.
