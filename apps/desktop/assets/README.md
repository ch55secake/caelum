# Home artwork

`black-hole-ascii.png` is a 2000×1122 copy of the supplied 5760×3232 Home artwork.
The desktop embeds the resized PNG and decodes it once on Rune's image worker.
Its single static image handle is reused across navigation and theme changes
(approximately 12 MiB of decoded pixels including mip levels).
