# Residue drill-down and atomic electron-view overlay

## Interaction

1. Load a PDB / mmCIF protein and switch to **3D molecular viewer**.
2. **Left-click** any residue in the protein trace, ribbon, or all-atoms view.
   The viewer selects the amino acid based on its imported chain, residue
   number, insertion code, and residue name, isolates its atomic coordinates,
   recenters the camera, and switches to the all-atoms representation.
3. **Left-click an atom** in the isolated amino acid. The view recenters on
   that atom, while retaining the surrounding residue. The details dock shows
   the atom name, element, formal charge, neutral atomic orbital configuration,
   and each directly bonded neighbor with bond order and configuration.
4. Click **SHOW ELECTRONS** in the right-hand dock to toggle illustrated 3D
   outer-shell atomic orbital clouds on the selected atom and its directly
   bonded atoms. Use the four independent **S / P / D / F** buttons under the
   toggle to show or hide each kind (all selected by default). The same button
   hides the overlay. For ordinary amino acids, D and F have no occupied
   outer orbitals and therefore correctly draw nothing; try a metal-containing
   structure for D, or a lanthanide/actinide structure for F. The dock works in
   previously implemented expanded viewer (**F**) and application fullscreen
   (**F11**) modes. Scroll inside the right-hand panel to inspect longer lists.
5. Click **EXIT FOCUS** to leave the isolated residue. **Escape** leaves
   residue focus when the viewer is in normal layout. Dragging still rotates,
   and Shift+drag or right-drag still pans.

Only mouse **clicks** (movement of at most 5 pixels between button press
and release) trigger atom/residue picking. Clicks on the toolbar or details
panel do not change the molecular selection. The picking algorithm uses the
current 3D rotation, zoom, camera pan and depth-dependent projected atom size;
protein ribbon/backbone segments also support click selection.

## Scientific scope

**This is an educational orbital-style diagram, not a quantum-mechanical
calculation of the molecular electron density.**

- Cyan sphere outlines approximate the spherical symmetry of occupied **s**
  valence orbitals; purple/pink paired lobes show occupied **p** directions.
- Green angular isosurfaces show occupied **d** orbitals: dxy, dxz, dyz,
  dx2-y2 and dz2. The four cloverleaf shapes and dz2's two lobes plus
  equatorial ring emerge from real spherical-harmonic angular functions.
- Orange angular isosurfaces show up to seven occupied **f** orientations,
  also generated from real (unnormalized) spherical-harmonic angular functions.
  Contrasting light/dark shades distinguish *phase*, not charge or spin.
- D/F orbitals use cached angular meshes at two polygon-detail levels.
  Coordinates rotate and scale with the molecule, with the mesh detail selected
  from apparent size in the viewport. The envelope is schematic and omits
  element-specific radial wavefunctions and any molecular-orbital calculation.
- Gold bond markers indicate the approximate region between bonded atoms.
  These are *not* measured shared-electron positions or calculated bond
  orbital probabilities.
- The dock lists **neutral, isolated-atom reference ground-state** electron
  configurations through lawrencium (Z <= 103), including the documented
  deviations from simple Aufbau filling in d/f-block elements. Long
  configurations are abbreviated using a preceding noble-gas core; e.g.
  Fe `[Ar] 4s2 3d6`, Ce `[Xe] 6s2 4f1 5d1`, U `[Rn] 7s2 5f3 6d1`.
  A molecule's actual electronic structure may
  differ substantially because of bonding, ionization, resonance and charge.
- Elements 104-118 are explicitly labeled **NOT MODELED** rather than
  predicting their configurations from an oversimplified filling order.
- The number and types of bonds come from the input and existing amino-acid
  bond-order templates. The overlay does **not** infer new covalent bonds.
- Missing atoms (e.g., hydrogens absent in crystallographic structures)
  remain missing except for any clearly flagged atoms that the existing
  optional reconstruction preview provides.

## Files changed

- `src/main.rs`: click/drag distinction, protein residue & atom selection,
  focused 3D inspector, electron overlay toggle, selection reset and shortcuts.
- `src/view.rs`: camera-projected picking, residue isolation, selected atom
  centering, and rendered 3D s/p/d/f orbital-style overlay.
- `src/electron.rs`: bonded neighborhood, neutral reference configurations
  and outer-shell s/p/d/f subshell occupations, with unit tests.
- `src/orbitals.rs`: cached signed angular isosurfaces for the five d and seven
  f orbitals, with nodal-shape and mesh tests.

No new dependencies. Uses the existing `mata-3d` CPU rasterizer and
`mata-win` framebuffer/UI. The original PDB/mmCIF data are not changed.

## Testing

Added tests for neutral reference configurations (H, C, N, O, S, Cr, Cu,
Se, Pd, Ce, Gd, U, Lr), conservation of electrons through Z=103, d/f shell
occupations, surface meshes, imported bond neighborhoods, focused picking/
recentering and camera pan rejection. Run from your Mata workspace:

```sh
cargo test -p mata-molecule
cargo run -p mata-molecule --release -- --open path/to/protein.cif
```

The archive is a single-crate component; building requires your local
sibling `../mata-win`, `../mata-3d`, and `../mata-fonts` crates. The Rust
compiler and those sibling crates are not present in the packaging environment,
so the changes have not been compilation-tested here.
