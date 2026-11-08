#!/usr/bin/env python3
"""Create the original, decorative, nonfunctional 'Astraeus LANCER-7' prop.

No dependencies. The model uses +Z forward and Y up, is in Mata gun view units,
uses diffuse/emissive MTL materials and triangulated OBJ (no texture downloads).
It is designed as videogame geometry, not as a working mechanism or blueprint.
"""
from math import pi,sin,cos,sqrt
from pathlib import Path

OUTPUT=Path(__file__).resolve().parent.parent/'assets'/'models'
OUTPUT.mkdir(parents=True, exist_ok=True)
NAME='gun_sniper'
PALETTE={
'carbon':((.065,.083,.105),False), 'deep_carbon':((.028,.040,.058),False),
'black_rubber':((.09,.095,.102),False), 'receiver':((.22,.26,.31),False),
'pale_armor':((.66,.66,.675),False), 'ivory':((.83,.80,.76),False),
'armor_shadow':((.38,.39,.43),False), 'chamfer':((.48,.52,.59),False),
'gunmetal':((.16,.19,.24),False), 'steel':((.45,.50,.56),False),
'titanium':((.59,.64,.70),False), 'warm_metal':((.48,.33,.26),False),
'copper':((.68,.43,.25),False), 'text':((.83,.90,.92),False),
'optic_black':((.018,.025,.039),False), 'optic_glass':((.11,.27,.38),False),
'blue_trim':((.10,.45,.76),False), 'cyan_light':((.20,.78,1.0),True),
'magenta_light':((.92,.23,.58),True), 'amber_light':((1.0,.58,.22),True),
'white_light':((.83,.93,1.0),True),
}
class Maker:
 def __init__(self):
  self.vertices=[];self.faces=[];self.cur='';self.group='';self.groups=[]
 def part(self,name):
  self.group=name
 def add(self,verts,faces,mat):
  start=len(self.vertices)+1
  self.vertices.extend(tuple(float(a) for a in point) for point in verts)
  for face in faces:
   self.faces.append((mat,self.group,tuple(start+i for i in face)))
 def box(self,x,y,z,w,h,d,mat,bevel=0.0):
  x0=x-w*.5; x1=x+w*.5;y0=y-h*.5;y1=y+h*.5;z0=z-d*.5;z1=z+d*.5
  v=[(x0,y0,z0),(x1,y0,z0),(x1,y1,z0),(x0,y1,z0),
     (x0,y0,z1),(x1,y0,z1),(x1,y1,z1),(x0,y1,z1)]
  self.add(v,[(0,2,1),(0,3,2),(4,5,6),(4,6,7),(0,4,7),(0,7,3),
              (1,2,6),(1,6,5),(3,7,6),(3,6,2),(0,1,5),(0,5,4)],mat)
  if bevel>0:
   # Raised, narrower inner-face plates give the silhouette visual chamfers.
   self.box(x,y,z,w*.92,h*.90,d*.985,'chamfer' if mat=='pale_armor' else mat)
 def poly_x(self,coords,xhalf,mat):
  """Decorative solid plate, convex polygon in the side view (z,y)."""
  # side polygon must be convex: thin strips and triangular overlays
  n=len(coords);assert n>=3
  v=[(-xhalf,y,z) for z,y in coords]+[(xhalf,y,z) for z,y in coords]
  fs=[]
  for i in range(1,n-1):fs.extend([(0,i+1,i),(n,n+i,n+i+1)])
  for j in range(n):k=(j+1)%n;fs.extend([(j,k,n+k),(j,n+k,n+j)])
  self.add(v,fs,mat)
 def tube_z(self,z0,z1,x,y,r0,r1,mat,n=12,open_ends=False):
  v=[]
  for z,r in ((z0,r0),(z1,r1)):
   v.extend((x+r*cos(2*pi*i/n),y+r*sin(2*pi*i/n),z) for i in range(n))
  fs=[]
  for i in range(n):
   j=(i+1)%n;fs.extend([(i,j,n+j),(i,n+j,n+i)])
  if not open_ends:
   for i in range(1,n-1):fs.extend([(0,i+1,i),(n,n+i,n+i+1)])
  self.add(v,fs,mat)
 def ring_z(self,z,x,y,rout,rin,length,mat,n=16):
  z0=z-length/2;z1=z+length/2;v=[]
  for zz,rr in [(z0,rout),(z1,rout),(z0,rin),(z1,rin)]:
   v.extend((x+rr*cos(2*pi*i/n),y+rr*sin(2*pi*i/n),zz) for i in range(n))
  fs=[]
  for i in range(n):
   j=(i+1)%n
   for a,b in [(0,1),(1,3),(3,2),(2,0)]:
    aa=i+n*a;ab=j+n*a;ba=i+n*b;bb=j+n*b
    fs.extend([(aa,ab,bb),(aa,bb,ba)])
  self.add(v,fs,mat)
 def tube_between(self,a,b,r,mat,n=9,tip=None):
  x,y,z=a;u,v,w=b
  dx=u-x;dy=v-y;dz=w-z;ll=sqrt(dx*dx+dy*dy+dz*dz)
  if ll<1e-5:return
  dr=(dx/ll,dy/ll,dz/ll)
  ref=(1.,0.,0.) if abs(dr[0])<.85 else (0.,1.,0.)
  p=(dr[1]*ref[2]-dr[2]*ref[1],dr[2]*ref[0]-dr[0]*ref[2],dr[0]*ref[1]-dr[1]*ref[0]);pn=sqrt(sum(t*t for t in p))
  p=tuple(t/pn for t in p)
  q=(dr[1]*p[2]-dr[2]*p[1],dr[2]*p[0]-dr[0]*p[2],dr[0]*p[1]-dr[1]*p[0])
  vtx=[]
  for cx,cy,cz,rr in [(x,y,z,r),(u,v,w,r if tip is None else tip)]:
   for i in range(n):
    a=2*pi*i/n;vtx.append((cx+rr*(p[0]*cos(a)+q[0]*sin(a)),cy+rr*(p[1]*cos(a)+q[1]*sin(a)),cz+rr*(p[2]*cos(a)+q[2]*sin(a))))
  faces=[]
  for i in range(n):
   j=(i+1)%n;faces.extend([(i,j,n+j),(i,n+j,n+i)])
  for i in range(1,n-1):faces.extend([(0,i+1,i),(n,n+i,n+i+1)])
  self.add(vtx,faces,mat)
 def facet(self,x,y,z,wx,hy,zz,mat):
  self.poly_x([(z-zz/2,y-hy/2),(z+zz/2,y-hy/2),(z+zz/2-.04,y+hy/2),(z-zz/2+.04,y+hy/2)],wx/2,mat)
 def save(self):
  materials=[f'# ASTRAEUS LANCER-7: {NAME}, original game-only stylized asset']
  for name,(kd,light) in PALETTE.items():
   materials.extend([f'newmtl {name}',f'Kd {kd[0]:.5f} {kd[1]:.5f} {kd[2]:.5f}'])
   if light:materials.append('Ke 0.72 0.72 0.72')
   materials.append('')
  (OUTPUT/f'{NAME}.mtl').write_text('\n'.join(materials)+'\n')
  out=[f'# ASTRAEUS LANCER-7 /// stylized futuristic sniper game prop',
   '# +Z forward, +Y up. Non-functional, decorative visual mesh.',
   f'mtllib {NAME}.mtl']
  out.extend('v %.6f %.6f %.6f'%point for point in self.vertices)
  old_mat=None;old_grp=None
  for mat,grp,face in self.faces:
   if grp!=old_grp:out.append('g '+grp);old_grp=grp
   if mat!=old_mat:out.append('usemtl '+mat);old_mat=mat
   assert len(face)==3
   out.append('f '+' '.join(map(str,face)))
  (OUTPUT/f'{NAME}.obj').write_text('\n'.join(out)+'\n')
  print('Generated:',len(self.vertices),'vertices,',len(self.faces),'triangles,',len(PALETTE),'materials,',len(set(g for _,g,_ in self.faces)),'detail groups')
  print('Mesh location:',OUTPUT)

m=Maker(); b=m.box; p=m.poly_x;t=m.tube_z;r=m.ring_z;beam=m.tube_between
# PROPORTIONS: stock -0.86 to -0.24; central receiver -0.27 .. 0.50,
# handguard 0.34 .. 1.17, barrel 1.12 .. 1.98; high scope Y 0.36
m.part('01_skeletal_adjustable_stock')
# outer housing and recoil buffer
b(0,-.05,-.77,.26,.26,.12,'deep_carbon')
b(0,-.06,-.725,.28,.28,.10,'black_rubber')
for z in (-.73,-.70,-.67):b(0,-.055,z,.285,.23,.025,'armor_shadow')
b(0,-.03,-.59,.21,.17,.17,'pale_armor')
# distinctive hollow skeleton cheekpad-support with diagonals and open centre
for xx in (-.095,.095):
 for a,ee in [((-0.10,-.49),(-.07,-.34)), ((-.12,-.66),(.02,-.49)),((.03,-.49),(-.10,-.34))]:
  z0,y0=a;z1,y1=ee;beam((xx,y0,z0),(xx,y1,z1),.022,'ivory',8)
beam((-.09,-.27,-.67),(-.09,.095,-.40),.022,'pale_armor')
beam((.09,-.27,-.67),(.09,.095,-.40),.022,'pale_armor')
b(0,.065,-.55,.25,.07,.32,'gunmetal')
b(0,.098,-.55,.24,.045,.30,'black_rubber')
# multiple positional notches and glowing adjustment marker
for z in [-.665,-.610,-.555,-.50]:b(-.126,-.045,z,.013,.025,.025,'steel')
b(0,-.105,-.47,.20,.17,.22,'pale_armor')
# STOCK PAINT PANELS
for sign in (-1,1):
 b(sign*.108,-.072,-.764,.010,.125,.057,'ivory')
 b(sign*.128,-.04,-.548,.015,.057,.17,'chamfer')

m.part('02_angled_grip_and_guard')
# angled grip / polygonal triggerguard external cosmetic with negative space
beam((0,-.095,-.33),(0,-.31,-.295),.043,'deep_carbon')
beam((0,-.30,-.30),(0,-.51,-.20),.052,'black_rubber')
b(0,-.40,-.235,.16,.21,.10,'black_rubber')
for j in range(5):b(0,-.333-j*.033,-.31+j*.017,.161,.009,.03,'armor_shadow')
for sign in (-1,1):
 beam((sign*.08,-.17,-.36),(sign*.08,-.37,-.40),.017,'pale_armor')
 beam((sign*.08,-.37,-.40),(sign*.08,-.40,-.21),.018,'pale_armor')
 beam((sign*.08,-.40,-.21),(sign*.08,-.20,-.14),.017,'pale_armor')

m.part('03_pale_angular_main_chassis')
# elongated monocoque with sloped facet overlays - purely ornamental geometry
b(0,-.070,.035,.37,.28,.77,'receiver')
b(0,-.080,-.010,.40,.255,.60,'pale_armor')
b(0,-.055,.260,.38,.28,.48,'ivory')
for side in (-1,1):
 x=side*.206
 b(x,-.087,.04,.027,.17,.54,'pale_armor')
 b(x,-.06,.36,.030,.21,.21,'armor_shadow')
 b(x,-.01,-.22,.026,.15,.12,'chamfer')
 for z in (-.26,-.15,.22,.34):
  b(side*.217,-.090,z,.006,.076,.019,'steel')
 # elongated angular motif two rising 'panel seams'
 beam((side*.226,-.10,-.10),(side*.231,-.010,.03),.007,'armor_shadow',6)
 beam((side*.230,-.005,.03),(side*.224,-.085,.17),.007,'armor_shadow',6)
 # engraved diamond logo
 beam((side*.235,-.04,-.235),(side*.235,.017,-.187),.005,'cyan_light',6)
 beam((side*.235,.017,-.187),(side*.235,-.04,-.139),.005,'cyan_light',6)
 beam((side*.235,-.04,-.139),(side*.235,-.092,-.187),.005,'cyan_light',6)
 beam((side*.235,-.092,-.187),(side*.235,-.04,-.235),.005,'cyan_light',6)
# bolt-like decorative shoulder channel line in upper dark rail
b(0,.100,.015,.280,.076,.70,'deep_carbon')
for z in [-.22+i*.043 for i in range(14)]:b(0,.147,z,.275,.015,.016,'gunmetal')
for xx in (-.14,.14):b(xx,.105,.014,.021,.052,.63,'steel')

m.part('04_floating_energy_feed')
# stylized power cell and horizontal ribbed 'coil' hidden in receiver cutout
b(.01,-.10,-.04,.23,.11,.40,'deep_carbon')
for j in range(23):
 z=-.22+j*.014
 t(z,z+.007,0,-.095,.082,.082,'copper' if j%4==0 else 'steel',10)
b(0,-.24,.09,.21,.22,.26,'deep_carbon')
b(0,-.26,.105,.20,.18,.21,'receiver')
for x in (-.085,.085):b(x,-.24,.110,.01,.13,.22,'titanium')
for z in (-.02,.05,.13,.20):b(.0,-.380,z,.12,.018,.018,'blue_trim')

m.part('05_magazine_and_magwell')
b(0,-.236,.295,.275,.145,.26,'carbon')
p([(.18,-.245),(.40,-.245),(.375,-.515),(.225,-.525)],.125,'deep_carbon')
b(0,-.415,.29,.26,.065,.20,'gunmetal')
for sign in [-1,1]:
 b(sign*.132,-.40,.30,.012,.20,.14,'armor_shadow')
 b(sign*.135,-.31,.30,.016,.018,.11,'cyan_light')
for i in range(5):b(0,-.50,.224+i*.038,.225,.015,.019,'receiver')

m.part('06_handguard_and_heatshield')
b(0,.002,.766,.365,.234,.85,'gunmetal')
b(0,-.023,.774,.38,.215,.82,'pale_armor')
b(0,-.055,.748,.40,.185,.79,'ivory')
# top and bottom spines
b(0,.135,.778,.24,.05,.83,'carbon')
b(0,-.164,.795,.258,.05,.71,'gunmetal')
for side in [-1,1]:
 sx=side*.206
 b(sx,-.015,.80,.020,.17,.77,'pale_armor')
 for j in range(11):
  z=.425+j*.059
  b(side*.218,-.009,z,.017,.085,.030,'deep_carbon')
  b(side*.229,-.014,z,.006,.066,.022,'armor_shadow')
 for z in (.420,.930,1.12):b(side*.215,-.123,z,.023,.037,.055,'titanium')
 # vents with bright strip at bottom
 b(side*.222,-.120,.74,.02,.025,.64,'cyan_light')
 # end caps
 b(side*.205,-.01,1.180,.03,.21,.12,'armor_shadow')
# top rail teeth
for j in range(21):b(0,.168,.37+j*.038,.242,.020,.020,'receiver')
for side in [-1,1]:b(side*.13,.175,.765,.020,.021,.80,'steel')

m.part('07_long_precision_barrel_and_muzzle')
# purely visual nested metallic cylinders/spirals
for z0,z1,rad,material,n in [(1.175,1.33,.10,'deep_carbon',14),(1.30,1.75,.057,'gunmetal',14),
 (1.75,1.81,.074,'steel',12),(1.81,1.945,.085,'gunmetal',16),(1.945,1.980,.107,'titanium',16)]:
 t(z0,z1,0,.004,rad,rad,material,n)
for z in [1.195,1.25,1.33,1.745,1.81,1.935]:r(z,0,.004,.103,.082,.025,'receiver',16)
for j in range(15):
 z=1.82+j*.008
 # muzzle outer scallop band; visually detailed small axial metallic accents
 for k in range(6):
  angle=2*pi*k/6
  x=.081*cos(angle);y=.004+.081*sin(angle)
  if j%3==0:beam((x,y,z),(x,y,z+.015),.008,'chamfer',6)
r(1.968,0,.004,.103,.056,.038,'gunmetal',20)
t(1.987,1.991,0,.004,.042,.042,'optic_black',16)

m.part('08_sight_optic_and_objective_lens')
# raised scope mount, front bell and rear eyepiece along +Z
b(0,.227,.162,.160,.17,.14,'carbon')
b(0,.253,.442,.164,.14,.14,'carbon')
for z in (.137,.437):
 b(0,.309,z,.22,.068,.07,'gunmetal')
 for x in (-.104,.104):b(x,.28,z,.025,.12,.09,'receiver')
# eyepiece at rear and larger tapered objective at forward end
for z0,z1,r0,r1,mat in [(-.23,-.10,.125,.134,'deep_carbon'),(-.10,.02,.138,.138,'receiver'),
 (.02,.42,.13,.13,'carbon'),(.42,.63,.15,.20,'gunmetal'),(.63,.69,.20,.202,'deep_carbon')]:
 t(z0,z1,0,.355,r0,r1,mat,20)
for zz,rad,thick,mat in [(-.22,.149,.041,'gunmetal'),(-.16,.145,.018,'titanium'),
 (.015,.144,.020,'chamfer'),(.20,.148,.026,'gunmetal'),(.39,.154,.018,'titanium'),
 (.57,.196,.022,'gunmetal'),(.66,.213,.055,'receiver'),(.691,.210,.016,'chamfer')]:
 r(zz,0,.355,rad,rad-.034,thick,mat,20)
# dark lenses with glint on both ends
for zz,rad in [(-.237,.114),(.694,.178)]:
 t(zz,zz+.001,0,.355,rad,rad,'optic_glass',20)
 r(zz,0,.355,rad*.95,rad*.81,.004,'blue_trim',20)
 beam((-.065,.405,zz-.001),(-.018,.438,zz-.001),.008,'cyan_light',6)
# parallax turret / adjustment rims, round silhouettes
for y0,y1,r0,mat in [(.490,.54,.078,'receiver'),(.54,.59,.087,'gunmetal'),(.59,.603,.08,'titanium')]:
 beam((0,y0,.215),(0,y1,.215),r0,mat,16)
# side knob
for xx in (-1,1):
 beam((xx*.12,.355,.205),(xx*.235,.355,.205),.075,'receiver',14)
 beam((xx*.225,.355,.205),(xx*.255,.355,.205),.083,'steel',14)
 for i in range(10):
  aa=2*pi*i/10
  x=xx*.25;y=.355+.07*cos(aa);z=.205+.07*sin(aa)
  b(x,y,z,.01,.018,.013,'carbon')

m.part('09_scope_brackets_and_diagnostics')
for z in (-.05,.40):
 for dx in (-.13,.13):beam((dx,.214,z),(dx*.97,.33,z),.022,'steel',9)
 r(z,0,.354,.157,.145,.035,'gunmetal',16)
# glowing side data strips (not inscriptions)
for sign in (-1,1):
 b(sign*.156,.348,.15,.011,.022,.23,'cyan_light')
 for i in range(7):b(sign*.158,.348,.052+i*.03,.014,.026,.008,'white_light')

m.part('10_folding_bipod_game_prop')
# distinct deployed bipod V silhouette, mounted near front of handguard
b(0,-.205,1.055,.27,.100,.17,'receiver')
for sign in (-1,1):
 a=(sign*.105,-.218,1.050);kn=(sign*.198,-.38,1.084);foot=(sign*.38,-.73,1.205)
 beam(a,kn,.044,'gunmetal',12)
 beam(kn,foot,.034,'carbon',12)
 beam((sign*.19,-.375,1.081),foot,.017,'steel',10)
 b(foot[0],foot[1]-.015,foot[2],.18,.05,.12,'black_rubber')
 for z in (1.06,1.08,1.10):b(sign*.198,-.389,z,.04,.03,.012,'titanium')
# center hinge light
b(0,-.254,1.054,.115,.028,.10,'cyan_light')

m.part('11_decorative_receiver_markings_and_panels')
for sign in (-1,1):
 # asymmetric contrast tabs / geometric branding on external surfaces
 b(sign*.23,-.13,-.075,.009,.022,.15,'warm_metal')
 b(sign*.218,-.016,.235,.009,.038,.08,'amber_light')
 for i in range(4):
  z=.16+i*.033
  b(sign*.228,-.154,z,.016,.014,.018,'deep_carbon')
 for z in (-.20,.05,.29):b(sign*.220,-.020,z,.012,.021,.020,'black_rubber')
# near-engraved accent strips on stock
b(0,.015,-.35,.165,.014,.14,'warm_metal')
b(0,.024,-.36,.132,.009,.09,'cyan_light')
# Tiny reflective registration marks and magenta designator glyphs.
b(.236,-.015,.272,.009,.012,.072,'text')
b(-.236,-.015,.272,.009,.012,.072,'text')
b(0,.141,-.268,.08,.012,.035,'magenta_light')

m.save()
