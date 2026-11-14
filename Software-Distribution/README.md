# NikPak — distribution kit

Version 1.13.2, released 2026-11-14. The declared download is 16.8 MB (17169 KB).

| File | What it is |
| --- | --- |
| `pad.xml` | PAD 4.0 — the file you submit |
| `pad-3.11.xml` | PAD 3.11 — for directories that still validate against it |
| `SUBMISSION.md` | the twenty directories, their rules and the category to pick |
| `submissions.csv` | tracker: one row per directory, fill the date and the result |
| `URLS.txt` | every URL the PAD declares, to paste into a form |
| `images/` | icon 32/64/128, logo 400, screenshots 4:3 and 16:9 |

## Hosting, once per product

Upload `pad.xml` to **https://nikpak.nikkoid.com/pad.xml** and keep that URL for every release. Directories poll it; if the URL changes, they open a second listing and the first one goes stale.

Upload the images to the paths the PAD names:

- `https://nikpak.nikkoid.com/pad/icon-64.png`
- `https://nikpak.nikkoid.com/pad/screenshots/nikpak-1024x768.png`

The other sizes in `images/` are for the sites whose forms ask for a 32x32 icon, a 256x256 icon, a 400x400 logo or a 16:9 screenshot.

## Each release

1. Edit `Software-Distribution/tools/projects.json` (version, date, change notes, sizes).
2. Run `python Software-Distribution/tools/build_pads.py nikpak`.
3. Upload `pad.xml` and `pad-3.11.xml` to the product site over the existing files.
4. Upload the new Windows/macOS/Linux archives to `downloads/`.
5. Run `python Software-Distribution/tools/check_pad.py` — it fetches every declared URL.

PAD-aware directories pick the new version up on their next crawl. The rest (MajorGeeks, SnapFiles, FileCroco, Softonic, MacUpdate, SourceForge, AlternativeTo, Software Informer) need a message through their form.

> Size note: Sum of nikpak.exe and unNik.exe; refresh once buildall packs the Windows zip.
