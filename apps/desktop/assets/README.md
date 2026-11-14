# Home artwork

Home uses `black-hole-ascii.png` in dark mode and `rocket-ascii.png` in light mode.
Both are 2000×1122 copies of the supplied 5760×3232 artwork.

The desktop embeds the resized PNGs and decodes each once on Rune's image worker.
Their static image handles are reused across navigation and theme changes
(under 24 MiB of decoded pixels in total, including mip levels).
