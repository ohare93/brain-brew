# Third-party fixture assets

Brain Brew's root `LICENSE` does **not** license the vendored Ultimate Geography
media wholesale.

The pinned fixture has 609 used media files with exactly one attribution owner:

- 548 Ultimate Geography image files (227 flags and 321 maps) are covered by
  `fixtures/ultimate-geography/sources.csv` and the license explanations in
  `fixtures/ultimate-geography/LICENSE.md`;
- 56 Hardcore Geography image files (39 flags and 17 maps) are covered by the
  separately pinned
  `fixtures/ultimate-geography-attribution/hardcore-geography/sources.csv`;
- the other five files are Ultimate Geography JavaScript/CSS runtime assets,
  covered by Ultimate Geography's deck/public-domain text and jsvectormap MIT
  notice in `fixtures/ultimate-geography/LICENSE.md`, as applicable.

The Hardcore supplement preserves exact `README.md` and `sources.csv` bytes from
`anki-geo/hardcore-geography` revision
`09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46`. That revision contains no
`LICENSE` or `NOTICE` file, and its README says only “Work In Progress.” Do not
infer a repository-wide license grant from the supplement: it records only the
per-file source, license label, and modification notes supplied for those 56
images. Ultimate Geography's `LICENSE.md` links the license terms referenced by
both source inventories; it does not broaden them.

The image records include public-domain, CC0, CC BY, and CC BY-SA material at
multiple license versions, and some assets may carry additional restrictions.
Consult the applicable pinned source row and upstream terms before copying or
redistributing fixture assets.

`media/ug-map-galapagos_islands.png` contains an inert
`file:///home/adam/Computing/foss_contributions/anki-ultimate-geography/...`
string in byte-preserved PNG metadata inherited from upstream. Brain Brew never
resolves or opens that URI; it is not a local input, runtime dependency, or
network reference. It remains solely because the fixture authenticates the
exact upstream bytes.
