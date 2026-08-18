# RambleDesk Windows installer

RambleDesk keeps Tauri's NSIS installation behavior and uses only NSIS Modern UI 2
features plus repository-owned artwork. Users can still choose the installation
directory.

There are no proprietary installer dependencies. A clean Windows CI runner can
build the installer through the normal Tauri CLI flow.

## Assets

NSIS MUI2's welcome sidebar is 164×314 and the header is 150×57 at 96 DPI.
Shipping those sizes 1:1 looks mosaicked on 150–200% displays, because Windows
stretches the bitmap to the DPI-scaled control. The checked-in bitmaps are
painted at 3× (492×942 and 450×171) so 200% and 300% stay sharp.

To rebuild the Modern UI artwork and the DMG background from brand assets:

```powershell
python .\generate-assets.py
```

`rambelle-cutout.png` is the transparent copy of the current Rambelle
assistant pose, shared with the installer sidebar. Keep that source and the
generated bitmaps in the same change when the character delivery is updated.

## Tauri upgrades

`rambledesk-installer.nsi` is based on Tauri CLI 2.11.4's upstream NSIS
template (same lineage as Kotone). When Tauri is upgraded, diff the new
upstream template and carry the RambleDesk MUI theme and copy changes forward
before shipping. Application installation, registry, shortcuts, WebView2,
reinstall, and uninstall behavior remain owned by Tauri's template.
