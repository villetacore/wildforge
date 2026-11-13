# Retro materials v1

Generated with built-in imagegen on 2026-11-11. Two source atlases are preserved
here. Equal cells were cropped with CoreGraphics (crop-atlas.swift) and resized to 256×256 PNG for
content/textures. Runtime use is diffuse color. The generated cells have not
been artist-retouched to guarantee perfect seamless edges.

## Atlas prompt

Production texture atlas for an original dark retro low-poly fantasy RPG.
Perfect square image subdivided into exactly 4 columns by 4 rows of equal square
cells, no gutters, no borders, no text. Each cell a flat orthographic evenly lit
diffuse material texture completely filling its square, coherent moderately
fine painted detail like early 2000s game textures, natural muted colors.
Row 1 left to right: grey limestone blocks; brown cobbled road; weathered dark
wooden planks; rough grey-beige fabric.
Row 2: dark old iron; cracked tree bark; muted olive leafy canopy texture opaque;
grey natural rock.
Row 3: muted green grass with soil; dark forest earth with sparse leaves;
grey-beige sand; grey rubble ground.
Row 4: grey wolf fur; pale old bone; dull purple-grey ogre skin; pale cyan
crystalline mineral.
Surfaces intended for tiled game meshes, no directional cast shadows, no
perspective, no photographed objects, no specular lighting, no giant pixel
blocks, no text or labels. Each cell cleanly separated only by its texture
change. Stylish worn hand-painted textures, usable asset sheet.

## Extra atlas prompt

Square production game diffuse texture atlas, exactly 3 columns by 3 rows equal
square cells, no gaps, no borders, no labels. Dark fantasy retro early 2000s
low-poly RPG materials with fine handpainted readable surface detail and natural
muted colors, even neutral lighting and straight orthographic surface view.
Row one left to right: neutral pale beige human skin surface without body parts;
dull grey green wet slime material; muted slate blue water with restrained tiny
ripples.
Row two: grey rock with rust colored iron ore veins; worn beige plaster with
localized cracks; weathered brown terracotta roof shingles.
Row three: dark brown leather with creases; faded ochre cloth with subtle folds;
brown rusty iron sheet with rivets.
Each cell filled edge to edge with the material only, no objects, no perspective,
no cast shadows, no text, no giant pixel blocks. Intended for tiling. The nine
textures must each cover precisely one cell.

## Mapping

atlas.png, row-major

stone_blocks, road, wood, cloth,
metal, bark, leaves, stone,
grass, forest, sand, rocky,
fur, bone, ogre_skin, crystal.

atlas-extra.png, row-major

skin, slime, water,
iron_ore, plaster, roof,
leather, cloak, rusted_iron.

Plaster, roof and rusted_iron are available for further architecture work.
Models are generated in client/rust/src/world/models.rs and structures.rs.
They use explicit planar normals and share generated material resources.
