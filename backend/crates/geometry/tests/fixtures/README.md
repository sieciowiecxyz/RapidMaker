# 3D Files For Testing

Tutaj trzymamy tylko wersjonowane pliki 3D do testów i fixture'y.

Nie wrzucać tutaj prawdziwych uploadów użytkowników.

## Co tu trafia

- Małe, deterministyczne modele referencyjne do parserów, slicera i preview.
- Pliki pozytywne dla STL, OBJ, PLY, GLB, 3MF i STEP:
  - `cube_20mm.stl` - binary STL
  - `cube_20mm_ascii.stl` - ASCII STL
  - `single_cube.obj`
  - `simple.ply`
  - `simple.glb`
  - `simple_model.3mf`
  - `step_block.step`
  - `step_cylinder.step`
- Pliki negatywne i regresyjne:
  - `invalid.stl`
  - `gltf_external_ref.gltf`
  - `step_unsupported_nurbs.step`
  - `zip_path_traversal.3mf`
  - `zip_duplicate_normalized.3mf`
  - `zip_external_relationship.3mf`
  - `zip_bomb_limit.3mf`
- Fixture'y skrajne do testów centrowania i fit-to-frame:
  - `tiny_cube_1mm.stl`,
  - `huge_cube_1000mm.stl`,
  - `off_center_cube.stl`.

## Zasady

- Każdy plik ma służyć testom, smoke albo regresji.
- Część plików jest mirrorem publicznych sample repo, część jest generowana lokalnie dla deterministycznych fixture'ów.
- Oryginalne uploady użytkowników nadal nie trafiają do tego katalogu.
