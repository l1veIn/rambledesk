# RambleDesk Windows installer

RambleDesk keeps Tauri's NSIS installation behavior and uses only NSIS Modern UI 2
features plus repository-owned artwork. Users can still choose the installation
directory.

There are no proprietary installer dependencies. A clean Windows CI runner can
build the installer through the normal Tauri CLI flow.

## Assets

NSIS MUI2's welcome sidebar is 164×314 and the header is 150×57. Shipping a
larger BMP does not help: NSIS copies the image into that control first, then
Windows stretches the copy. Oversized headers have also failed to load on the
page after Welcome. The generator paints at 3× and Lanczos-resamples to those
exact sizes so the checked-in files stay NSIS-compatible.

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
