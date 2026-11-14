# Home artwork

Home uses `black-hole-ascii.png` in dark mode and `clouds-sunlit-ascii.png` in light mode.
Both are 2000×1122 images derived from the supplied artwork. The clouds use an ivory
background and blue-grey characters, with light-background glyph density to keep
the sunlit highlights pale.

The desktop embeds the PNGs and decodes each once on Rune's image worker.
Their static image handles are reused across navigation and theme changes
(under 24 MiB of decoded pixels in total, including mip levels).
