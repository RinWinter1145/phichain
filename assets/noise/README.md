# Phigros noise-domain preview assets

These textures are extracted from the official game package supplied for compatibility testing.
They are used only by the `blockAreaList` preview renderer:

- `FD_Noise_00000.png` — active/touch color noise (`sharedassets12.assets`, path ID 15)
- `Block.png` — block mask sprite (path ID 16)
- `PointNoise.png` — disabled-domain sparks (path ID 17)
- `BlockNoise1.png` — displacement map (path ID 30)

The runtime material in `assets/shaders/noise_composite.wgsl` follows the official material
parameters for `ActiveBlock`, `DisabledBlock`, `ReadyBlock`, `BlockCompose`, and
`SubtractBlockBlender`. Editing outlines and handles are rendered separately and are not part of
the exported chart or gameplay preview.
