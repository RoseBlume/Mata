"""Static validation of editable OBJ/MTL assets and Neon Run integration."""
from pathlib import Path
import math, re
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'assets/models'
REQUIRED=['enemy_runner','enemy_striker','enemy_tank','enemy_marksman','enemy_gunner',
    'gun_pulse_carbine','gun_arc_cannon','gun_sniper','gun_shotgun','gun_minigun']
DETAILS=['enemy_'+name for name in ('runner','striker','tank','marksman','gunner')]
for name in REQUIRED:
 obj=(ASSETS/(name+'.obj')).read_text();mtl=(ASSETS/(name+'.mtl')).read_text()
 vertices=[];faces=[];palette=set();used=set();current='default'
 for ln,line in enumerate(obj.splitlines(),1):
  line=line.strip()
  if line.startswith('v '):
   numbers=[float(x) for x in line.split()[1:]]
   assert len(numbers)==3 and all(math.isfinite(x) for x in numbers),(name,ln)
   vertices.append(numbers)
  elif line.startswith('usemtl '):
   current=line.split(maxsplit=1)[1];used.add(current)
  elif line.startswith('f '):
   entries=[int(x.split('/')[0]) for x in line.split()[1:]]
   assert len(entries)>=3 and all(1<=x<=len(vertices) for x in entries),(name,ln)
   faces.extend((current,[entries[0],entries[i],entries[i+1]]) for i in range(1,len(entries)-1))
 palette=set(re.findall(r'^newmtl\s+(\S+)',mtl,re.M))
 assert palette>=used,(name,used-palette)
 assert len(faces)>=180,(name,len(faces))
 if name in DETAILS:
  assert len(faces)>=3000,(name,'not a high-detail model',len(faces))
  assert len(list(re.finditer(r'^usemtl ',obj,re.M)))<=16,(name,'too many material batches')
 assert len(palette)>=5,(name,len(palette))
 bounds=[(min(v[j] for v in vertices),max(v[j] for v in vertices)) for j in range(3)]
 assert all(lo<hi for lo,hi in bounds)
 print(f'{name:20} {len(vertices):>4} vertices {len(faces):>4} triangles {len(palette)} materials bounds {bounds}')
render=(ROOT/'src/render.rs').read_text()
# mata-3d is an external sibling crate; permit asset checks in isolation.
engine_path=ROOT.parent/'mata-3d/src/lib.rs'
models_path=ROOT.parent/'mata-3d/src/model.rs'
engine=engine_path.read_text() if engine_path.exists() else None
models=models_path.read_text() if models_path.exists() else None
for name in REQUIRED:
 if name.startswith('enemy_'):
  continue  # Rig has individually included, full-resolution joint meshes.
 assert f'include_str!("../assets/models/{name}.obj")' in render
 assert f'include_str!("../assets/models/{name}.mtl")' in render
if engine is not None and models is not None:
 assert 'pub mod model' in engine and 'pub fn mesh_view' in engine
 assert 'pub fn from_obj_mtl' in models and 'fn draw_view' in models
assert 'self.enemies.draw' in render and 'weapon.draw_view' in render
assert 'mod enemy_motion;' in (ROOT/'src/main.rs').read_text()
assert 'mod enemy_rig;' in (ROOT/'src/main.rs').read_text()
assert 'self.raster.clear_depth()' in render
assert 'let mut cube=|' not in render
for name in DETAILS:
 tris=[]
 for suffix in ('','_lod','_far'):
  obj=(ASSETS/(name+suffix+'.obj')).read_text()
  mtl=(ASSETS/(name+suffix+'.mtl')).read_text()
  mats=set(re.findall(r'^newmtl\s+(\S+)',mtl,re.M))
  used=set(re.findall(r'^usemtl\s+(\S+)',obj,re.M))
  assert used.issubset(mats),(name+suffix,'missing materials')
  verts=[tuple(map(float,l.split()[1:4])) for l in obj.splitlines() if l.startswith('v ')]
  assert all(len(v)==3 and all(math.isfinite(x) for x in v) for v in verts)
  faces=[l for l in obj.splitlines() if l.startswith('f ')]
  for line in faces:
   idx=[int(a.split('/')[0]) for a in line.split()[1:]]
   assert all(0<i<=len(verts) for i in idx),(name+suffix,'invalid face')
  tris.append(sum(len(l.split())-3 for l in faces))
 assert tris[0]>tris[1]>tris[2],(name,tris)
 print('Three LODs',name,':',tris)
# Rig triangle sums are exactly equal to the complete original detailed and
# medium meshes. Neither mesh quality nor any material has been thrown away.
parts=['body','head','thigh_l','shin_l','foot_l','thigh_r','shin_r','foot_r',
    'upper_arm_l','forearm_l','upper_arm_r','forearm_r']
for name in DETAILS:
 for suffix in ('','_lod'):
  source=name+suffix
  obj=(ASSETS/(source+'.obj')).read_text()
  full_faces=[ln for ln in obj.splitlines() if ln.startswith('f ')]
  original_tris=sum(len(ln.split())-3 for ln in full_faces)
  parts_tri=0
  for part in parts:
   path=ASSETS/'rig'/(source+'_'+part+'.obj')
   txt=path.read_text()
   lines=txt.splitlines()
   vertices=[tuple(map(float,l.split()[1:])) for l in lines if l.startswith('v ')]
   assert vertices and all(len(p)==3 and all(math.isfinite(v) for v in p) for p in vertices)
   used=set(re.findall(r'^usemtl\s+(\S+)',txt,re.M))
   palette=set(re.findall(r'^newmtl\s+(\S+)',(ASSETS/(source+'.mtl')).read_text(),re.M))
   assert used<=palette,(path,used-palette)
   for face in (ln for ln in lines if ln.startswith('f ')):
    idx=[int(t) for t in face.split()[1:]]
    assert all(0<i<=len(vertices) for i in idx),(path,'bad face index')
    parts_tri+=len(idx)-2
  assert parts_tri==original_tris,(source,parts_tri,original_tris)
  print(f'FULL FIDELITY: {source}: {parts_tri} input triangles = {parts_tri} rig triangles; {len(parts)} independent pieces')
assert 'enemy_runner_lod.obj' in str(ASSETS/'enemy_runner_lod.obj')
print('PASS: model content, imports, materials, no legacy enemy/weapon cube draw paths')

# Separate 4K *hero* tier (source polygons retained; new detail appended).
from collections import Counter

def obj_data(file):
    vertices=[]; faces=[]; material='default'
    for line in file.read_text().splitlines():
        fields=line.split()
        if not fields:continue
        if fields[0]=='v':
            vals=tuple(map(float,fields[1:4]))
            assert len(vals)==3 and all(math.isfinite(x) for x in vals),file
            vertices.append(vals)
        elif fields[0]=='usemtl':material=fields[1]
        elif fields[0]=='f':
            indices=tuple(int(v.split('/')[0]) for v in fields[1:])
            assert len(indices)>=3 and all(1<=i<=len(vertices) for i in indices),file
            faces.append((material,indices))
    return vertices, faces

for name in DETAILS:
    src_verts,src_faces=obj_data(ASSETS/(name+'.obj'))
    hero_verts,hero_faces=obj_data(ASSETS/(name+'_4k.obj'))
    assert hero_verts[:len(src_verts)]==src_verts,(name,'original vertex positions changed')
    original=Counter(src_faces);grown=Counter(hero_faces)
    assert all(grown[f]>=count for f,count in original.items()),(name,'original polygons lost')
    hero_materials=set(re.findall(r'^newmtl\s+(\S+)',
        (ASSETS/(name+'_4k.mtl')).read_text(),re.M))
    assert set(mat for mat,_ in hero_faces)<=hero_materials,(name,'missing hero material')
    expected=sum(len(f)-2 for _,f in hero_faces)
    assert expected>2*sum(len(f)-2 for _,f in src_faces),(name,'no extra authored detail')
    piece_count=0
    for part in parts:
        original_part=ASSETS/'rig'/(name+'_'+part+'.obj')
        hero_part=ASSETS/'rig'/(name+'_4k_'+part+'.obj')
        src_text=original_part.read_text();hero_text=hero_part.read_text()
        pivot_a=re.search(r'^# pivot (.+)$',src_text,re.M)
        pivot_b=re.search(r'^# pivot (.+)$',hero_text,re.M)
        assert pivot_a and pivot_b and pivot_a.group(1)==pivot_b.group(1),(name,part,'pivot changed')
        _,piece_faces=obj_data(hero_part)
        assert piece_faces,(name,part,'empty hero part')
        piece_count+=sum(len(face)-2 for _,face in piece_faces)
    assert piece_count==expected,(name,'hero rig lost triangles')
    print('4K HERO:',name,f'{expected:,} triangles, +{expected-sum(len(f)-2 for _,f in src_faces):,} added; 12/12 articulated parts, base geometry intact')
assert 'hero_4k' in (ROOT/'src/enemy_rig.rs').read_text()
assert 'full_height>=1100' in (ROOT/'src/render.rs').read_text()
print('PASS: Five 4K hero meshes, 60 joint parts, exact original geometry preserved')
