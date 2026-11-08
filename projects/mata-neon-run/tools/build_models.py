"""Build original, low-poly sci-fi OBJ/MTL assets for Neon Run.

All measurements are fictional game units. Models intentionally have broad
silhouettes, separate luminescent parts, and no textures/dependencies.
Run from the project root to regenerate editable Blender-compatible meshes.
"""
from pathlib import Path
import math
from collections import OrderedDict
OUT=Path(__file__).resolve().parents[1]/'assets/models'
OUT.mkdir(exist_ok=True,parents=True)

class Model:
 def __init__(self,name):
  self.name=name;self.v=[];self.faces=[];self.materials=OrderedDict()
 def material(self,name,rgb,glow=False):
  self.materials[name]=(rgb,glow)
 def vert(self,v):
  self.v.append(tuple(v));return len(self.v)
 def face(self,mat,points):
  self.faces.append((mat,tuple(points)))
 def box(self,mat,c,s):
  x,y,z=c;dx,dy,dz=(t*0.5 for t in s)
  lo=(x-dx,y-dy,z-dz);hi=(x+dx,y+dy,z+dz)
  vv=[self.vert(v) for v in [(lo[0],lo[1],lo[2]),(hi[0],lo[1],lo[2]),(hi[0],hi[1],lo[2]),(lo[0],hi[1],lo[2]),(lo[0],lo[1],hi[2]),(hi[0],lo[1],hi[2]),(hi[0],hi[1],hi[2]),(lo[0],hi[1],hi[2])]]
  for f in [(0,3,2,1),(4,5,6,7),(0,4,7,3),(1,2,6,5),(3,7,6,2),(0,1,5,4)]:self.face(mat,[vv[i] for i in f])
 def taper_box(self,mat,y0,y1,xz0,xz1,cx=0.0,cz=0.0):
  # Low-poly armour prism with two different rectangular cross-sections.
  verts=[]
  for y,(rx,rz) in [(y0,xz0),(y1,xz1)]:
   for x,z in [(-rx,-rz),(rx,-rz),(rx,rz),(-rx,rz)]:verts.append(self.vert((cx+x,y,cz+z)))
  self.face(mat,[verts[3],verts[2],verts[1],verts[0]]);self.face(mat,verts[4:])
  for i in range(4):self.face(mat,[verts[i],verts[(i+1)%4],verts[4+(i+1)%4],verts[4+i]])
 def bevel_box(self,mat,c,s,bevel=0.1):
  # 8-corner chamfered footprint extruded vertically. The shape's forward
  # surface and bevels remain visible at quarter-resolution rendering.
  x,y,z=c;hw,hh,hd=[u*0.5 for u in s]
  b=min(bevel,hw*0.48,hd*0.48)
  polygon=[(-hw+b,-hd),(hw-b,-hd),(hw,-hd+b),(hw,hd-b),(hw-b,hd),(-hw+b,hd),(-hw,hd-b),(-hw,-hd+b)]
  bottom=[self.vert((x+xx,y-hh,z+zz)) for xx,zz in polygon]
  top=[self.vert((x+xx,y+hh,z+zz)) for xx,zz in polygon]
  for i in range(1,7):
   self.face(mat,[bottom[0],bottom[i+1],bottom[i]]);self.face(mat,[top[0],top[i],top[i+1]])
  for i in range(8):
   j=(i+1)%8;self.face(mat,[bottom[i],bottom[j],top[j],top[i]])
 def cylinder(self,mat,a,b,r1,r2=None,n=8):
  """Cylinder between points, with oriented ring normals."""
  if r2 is None:r2=r1
  delta=[b[i]-a[i] for i in range(3)];length=math.sqrt(sum(t*t for t in delta))
  if length<1e-8:return
  axis=[v/length for v in delta]
  ref=[0,1,0] if abs(axis[1])<0.90 else [1,0,0]
  ux=[axis[1]*ref[2]-axis[2]*ref[1],axis[2]*ref[0]-axis[0]*ref[2],axis[0]*ref[1]-axis[1]*ref[0]]
  um=math.sqrt(sum(t*t for t in ux));ux=[x/um for x in ux]
  uy=[axis[1]*ux[2]-axis[2]*ux[1],axis[2]*ux[0]-axis[0]*ux[2],axis[0]*ux[1]-axis[1]*ux[0]]
  rings=[]
  for center,radius in ((a,r1),(b,r2)):
   rings.append([self.vert(tuple(center[i]+radius*(ux[i]*math.cos(t*2*math.pi/n)+uy[i]*math.sin(t*2*math.pi/n)) for i in range(3))) for t in range(n)])
  for i in range(1,n-1):
   self.face(mat,[rings[0][0],rings[0][i+1],rings[0][i]])
   self.face(mat,[rings[1][0],rings[1][i],rings[1][i+1]])
  for i in range(n):j=(i+1)%n;self.face(mat,[rings[0][i],rings[0][j],rings[1][j],rings[1][i]])
 def octa(self,mat,c,s):
  x,y,z=c;dx,dy,dz=s
  vv=[self.vert((x+dx,y,z)),self.vert((x-dx,y,z)),self.vert((x,y+dy,z)),self.vert((x,y-dy,z)),self.vert((x,y,z+dz)),self.vert((x,y,z-dz))]
  for f in [(0,2,4),(2,1,4),(1,3,4),(3,0,4),(2,0,5),(1,2,5),(3,1,5),(0,3,5)]:self.face(mat,[vv[i] for i in f])
 def arrow(self,mat,c,s):
  x,y,z=c;dx,dy,dz=s
  vv=[self.vert(p) for p in [(x-dx,y-dy,z-dz),(x+dx,y-dy,z-dz),(x,y+dy,z-dz),(x,y,z+dz)]]
  for f in [(0,1,2),(0,3,1),(1,3,2),(2,3,0)]:self.face(mat,[vv[i] for i in f])
 def save(self):
  obj=['# MATA NEON RUN - Original low-poly game model',f'# {self.name}',f'mtllib {self.name}.mtl','']
  for v in self.v:obj.append('v '+' '.join(f'{n:.6f}' for n in v))
  last=None
  for mat,face in self.faces:
   if mat!=last:obj.append(f'usemtl {mat}');last=mat
   obj.append('f '+' '.join(str(x) for x in face))
  (OUT/(self.name+'.obj')).write_text('\n'.join(obj)+'\n')
  mtl=['# Flat-colour PBR-free materials for Mata software rasterizer']
  for name,(col,glow) in self.materials.items():
   mtl.append('newmtl '+name)
   mtl.append('Kd '+' '.join(f'{c/255:.6f}' for c in col))
   if glow:mtl.append('Ke 0.750000 0.750000 0.750000')
   mtl.append('')
  (OUT/(self.name+'.mtl')).write_text('\n'.join(mtl)+'\n')
  used=set(mat for mat,_ in self.faces)
  assert used.issubset(self.materials), (self.name,used-set(self.materials))
  print(self.name,len(self.v),'vertices',len(self.faces),'polygons',len(used),'materials')


def palette(m):
 m.material('dark',(21,33,47));m.material('metal',(54,75,92));m.material('joint',(32,45,61));m.material('silver',(112,137,151));m.material('light',(155,205,225))

def build_runner():
 m=Model('enemy_runner');palette(m)
 m.material('orange',(241,136,43));m.material('ochre',(145,85,42));m.material('cyan',(65,246,250),True);m.material('ember',(255,184,79),True)
 # Tiny, narrow, sprinter-shaped biped. All faces visible without textures.
 m.taper_box('metal',0.97,1.45,(0.29,0.18),(0.34,0.24))
 m.bevel_box('orange',(0,1.31,0.15),(0.57,0.31,0.20),0.08)
 m.box('silver',(0,1.32,0.267),(0.18,0.08,0.035))
 m.bevel_box('joint',(0,1.64,0.0),(0.34,0.29,0.34),0.06)
 m.bevel_box('orange',(0,1.71,0.07),(0.42,0.25,0.36),0.065)
 m.box('cyan',(0,1.715,0.253),(0.30,0.09,0.033))
 for sign in (-1,1):
  x=sign*0.37
  m.cylinder('joint',(sign*0.31,1.38,0),(sign*0.42,1.12,0.07),0.13,n=6)
  m.cylinder('metal',(sign*0.42,1.12,0.07),(sign*0.42,0.78,0.23),0.10,n=6)
  m.bevel_box('orange',(sign*0.42,1.27,0.05),(0.22,0.33,0.24),0.045)
  m.cylinder('joint',(sign*0.42,0.80,0.05),(sign*0.36,0.79,0.05),0.10,n=6)
  m.cylinder('metal',(sign*0.20,0.98,0),(sign*0.25,0.58,-0.05),0.14,n=6)
  m.cylinder('silver',(sign*0.25,0.58,-0.05),(sign*0.27,0.18,0.12),0.105,0.08,n=6)
  m.bevel_box('dark',(sign*0.27,0.11,0.20),(0.26,0.18,0.48),0.07)
  m.bevel_box('ochre',(sign*0.24,0.70,0.12),(0.23,0.28,0.23),0.05)
  m.cylinder('cyan',(sign*0.33,1.12,-0.20),(sign*0.35,1.12,-0.30),0.075,n=6)
  m.arrow('ember',(sign*0.35,1.12,-0.36),(0.07,0.07,0.12))
 m.arrow('orange',(0,1.93,0),(0.10,0.18,0.15))
 m.save()


def build_striker():
 m=Model('enemy_striker');palette(m)
 m.material('magenta',(235,61,140));m.material('hot',(255,104,194),True);m.material('violet',(122,64,154));m.material('glass',(114,249,249),True)
 # Armoured arena hunter: wide pincer arms, angular central head and winglets.
 m.taper_box('violet',0.90,1.52,(0.46,0.27),(0.39,0.28))
 m.bevel_box('metal',(0,1.29,-0.01),(0.93,0.48,0.61),0.15)
 m.bevel_box('magenta',(0,1.29,0.30),(0.72,0.28,0.13),0.065)
 m.arrow('hot',(0,1.30,0.385),(0.20,0.19,0.10))
 m.taper_box('dark',1.53,1.88,(0.29,0.24),(0.19,0.18),cz=0.1)
 m.bevel_box('violet',(0,1.72,0.15),(0.43,0.31,0.40),0.08)
 m.box('glass',(0,1.72,0.361),(0.36,0.075,0.04))
 for sign in (-1,1):
  m.bevel_box('magenta',(sign*0.53,1.47,0),(0.48,0.30,0.42),0.10)
  m.arrow('violet',(sign*0.77,1.52,-0.05),(0.16,0.28,0.25))
  m.cylinder('joint',(sign*0.71,1.32,0.1),(sign*0.83,0.89,0.14),0.15,n=6)
  m.bevel_box('metal',(sign*0.83,0.79,0.19),(0.31,0.46,0.31),0.07)
  m.arrow('hot',(sign*0.89,0.45,0.40),(0.16,0.47,0.52))
  m.cylinder('joint',(sign*0.27,0.97,0),(sign*0.29,0.55,-0.03),0.17,n=6)
  m.bevel_box('violet',(sign*0.29,0.38,0.08),(0.36,0.49,0.38),0.06)
  m.bevel_box('dark',(sign*0.28,0.09,0.19),(0.34,0.16,0.53),0.08)
  m.cylinder('hot',(sign*0.51,1.25,-0.26),(sign*0.51,1.25,-0.35),0.085,n=6)
 m.save()


def build_tank():
 m=Model('enemy_tank');palette(m)
 m.material('purple',(153,112,209));m.material('plating',(82,68,126));m.material('core',(237,128,255),True);m.material('indicator',(136,242,253),True)
 # Heavy two-legged fortress, thick shoulder shields, back energy tanks.
 m.taper_box('dark',0.85,2.00,(0.63,0.42),(0.69,0.42))
 m.bevel_box('plating',(0,1.60,0.04),(1.47,0.94,0.88),0.21)
 m.bevel_box('purple',(0,1.74,0.49),(1.04,0.53,0.13),0.055)
 m.bevel_box('dark',(0,1.74,0.59),(0.61,0.41,0.07),0.04)
 m.octa('core',(0,1.75,0.65),(0.23,0.24,0.07))
 m.bevel_box('metal',(0,2.24,-0.08),(0.71,0.38,0.57),0.13)
 m.bevel_box('purple',(0,2.36,0.08),(0.60,0.27,0.46),0.10)
 m.box('indicator',(0,2.36,0.326),(0.51,0.08,0.04))
 m.box('silver',(0,2.10,0.24),(0.31,0.08,0.24))
 for sign in (-1,1):
  m.bevel_box('plating',(sign*0.82,1.91,-0.06),(0.53,0.59,0.89),0.13)
  m.bevel_box('purple',(sign*0.90,2.10,0.04),(0.49,0.26,0.74),0.08)
  m.cylinder('joint',(sign*0.87,1.66,0.07),(sign*0.99,1.21,0.18),0.24,n=8)
  m.bevel_box('dark',(sign*1.03,1.11,0.28),(0.43,0.68,0.60),0.10)
  m.box('core',(sign*1.03,1.11,0.597),(0.17,0.18,0.04))
  m.cylinder('joint',(sign*0.44,0.99,-0.07),(sign*0.51,0.54,0.01),0.26,n=8)
  m.bevel_box('plating',(sign*0.51,0.40,0.09),(0.49,0.61,0.49),0.12)
  m.bevel_box('dark',(sign*0.52,0.11,0.31),(0.56,0.22,0.79),0.12)
  m.cylinder('silver',(sign*0.60,1.85,-0.46),(sign*0.60,2.19,-0.59),0.18,n=8)
  m.cylinder('core',(sign*0.60,2.20,-0.62),(sign*0.60,2.26,-0.64),0.105,n=8)
  m.box('indicator',(sign*0.75,2.12,0.45),(0.16,0.055,0.035))
 m.save()


def build_pulse_carbine():
 m=Model('gun_pulse_carbine');palette(m)
 m.material('shell',(46,67,91));m.material('cyan',(74,242,255),True);m.material('trigger',(12,22,35));m.material('trim',(109,155,177))
 # Offset first-person sci-fi rifle, pointing along +Z.
 m.bevel_box('shell',(0,-0.01,0.08),(0.44,0.28,0.96),0.08)
 m.bevel_box('metal',(0,0.08,0.24),(0.42,0.11,0.62),0.035)
 m.bevel_box('dark',(0,-0.21,-0.19),(0.19,0.43,0.25),0.04)
 m.box('trigger',(0,-0.21,-0.03),(0.20,0.16,0.11))
 m.bevel_box('trim',(0,0.18,-0.12),(0.21,0.14,0.31),0.05)
 m.box('cyan',(0,0.260,0.04),(0.11,0.034,0.31))
 m.bevel_box('shell',(0,-0.075,0.56),(0.32,0.24,0.50),0.065)
 m.cylinder('dark',(0,0.03,0.61),(0,0.03,1.12),0.105,n=10)
 m.cylinder('cyan',(0,0.03,1.07),(0,0.03,1.13),0.11,n=10)
 m.cylinder('silver',(0,0.03,1.09),(0,0.03,1.155),0.063,n=10)
 for sign in (-1,1):
  m.bevel_box('trim',(sign*0.205,-0.01,0.15),(0.07,0.24,0.53),0.018)
  m.box('cyan',(sign*0.25,0.01,0.19),(0.022,0.06,0.31))
  m.cylinder('shell',(sign*0.138,0.01,0.56),(sign*0.138,0.01,0.92),0.055,n=6)
 m.save()


def build_arc_cannon():
 m=Model('gun_arc_cannon');palette(m)
 m.material('shell',(69,64,105));m.material('violet',(158,104,236));m.material('energy',(252,113,237),True);m.material('core',(126,254,255),True)
 m.bevel_box('shell',(0,0.015,0.12),(0.60,0.37,0.76),0.13)
 m.bevel_box('violet',(0,0.195,0.15),(0.55,0.11,0.58),0.07)
 m.bevel_box('joint',(0,-0.245,-0.08),(0.21,0.39,0.25),0.055)
 m.bevel_box('metal',(0,-0.05,0.55),(0.52,0.28,0.49),0.09)
 m.bevel_box('dark',(0,0.12,0.63),(0.35,0.25,0.56),0.08)
 m.cylinder('energy',(0,0.12,0.64),(0,0.12,0.92),0.125,n=8)
 for sign in (-1,1):
  m.bevel_box('shell',(sign*0.285,0.05,0.36),(0.19,0.24,0.66),0.056)
  m.cylinder('metal',(sign*0.205,0.02,0.70),(sign*0.205,0.02,1.19),0.11,n=10)
  m.cylinder('core',(sign*0.205,0.02,1.19),(sign*0.205,0.02,1.23),0.094,n=10)
  m.cylinder('energy',(sign*0.31,0.11,0.33),(sign*0.31,0.11,0.58),0.045,n=6)
  m.box('core',(sign*0.32,0.175,0.10),(0.042,0.072,0.23))
 m.bevel_box('violet',(0,0.290,0.02),(0.28,0.12,0.30),0.045)
 m.box('core',(0,0.35,0.02),(0.10,0.025,0.17))
 m.save()

for fun in [build_runner,build_striker,build_tank,build_pulse_carbine,build_arc_cannon]:fun()

def build_lod_runner():
 m=Model('enemy_runner_lod');palette(m)
 m.material('orange',(241,136,43));m.material('cyan',(65,246,250),True)
 m.box('metal',(0,1.2,0),(0.52,0.69,0.36))
 m.box('orange',(0,1.43,0.14),(0.51,0.23,0.13))
 m.box('orange',(0,1.72,0.02),(0.34,0.28,0.32))
 m.box('cyan',(0,1.72,0.195),(0.28,0.08,0.04))
 for k in (-1,1):
  m.box('metal',(k*0.35,1.06,0.02),(0.18,0.53,0.24))
  m.box('dark',(k*0.19,0.38,0),(0.22,0.67,0.27))
  m.box('dark',(k*0.2,0.10,0.21),(0.24,0.15,0.43))
 m.save()

def build_lod_striker():
 m=Model('enemy_striker_lod');palette(m)
 m.material('magenta',(235,61,140));m.material('hot',(255,104,194),True)
 m.box('metal',(0,1.25,0),(0.85,0.59,0.52))
 m.box('magenta',(0,1.22,0.29),(0.65,0.24,0.10))
 m.box('magenta',(0,1.70,0.06),(0.43,0.31,0.33))
 m.box('hot',(0,1.72,0.24),(0.34,0.08,0.03))
 for k in (-1,1):
  m.box('magenta',(k*0.57,1.39,0),(0.40,0.27,0.41))
  m.box('metal',(k*0.82,0.86,0.14),(0.24,0.60,0.30))
  m.arrow('hot',(k*0.88,0.46,0.39),(0.16,0.32,0.37))
  m.box('dark',(k*0.28,0.45,0.02),(0.30,0.83,0.35))
 m.save()

def build_lod_tank():
 m=Model('enemy_tank_lod');palette(m)
 m.material('purple',(153,112,209));m.material('core',(237,128,255),True)
 m.box('dark',(0,1.6,0),(1.37,1.1,0.85))
 m.box('purple',(0,1.79,0.45),(1.07,0.55,0.12))
 m.box('core',(0,1.75,0.53),(0.33,0.34,0.025))
 m.box('purple',(0,2.36,0.04),(0.54,0.29,0.48))
 m.box('core',(0,2.37,0.30),(0.44,0.09,0.028))
 for k in (-1,1):
  m.box('purple',(k*0.87,1.94,0),(0.52,0.61,0.76))
  m.box('metal',(k*0.94,1.23,0.13),(0.30,0.66,0.35))
  m.box('dark',(k*0.48,0.46,0.08),(0.43,0.86,0.46))
  m.box('dark',(k*0.49,0.12,0.24),(0.54,0.21,0.72))
 m.save()

for fun in [build_lod_runner,build_lod_striker,build_lod_tank]:fun()

# --- Extended five-weapon release: two more enemy classes and three guns. ---
def build_sniper():
 m=Model('gun_sniper');palette(m)
 m.material('shell',(36,56,70));m.material('rail',(127,170,187));m.material('glass',(84,217,255),True);m.material('coil',(100,247,217),True)
 m.bevel_box('shell',(0,-0.02,0.16),(0.30,0.24,0.95),0.05)
 m.bevel_box('metal',(0,-0.21,-0.20),(0.15,0.36,0.22),0.035)
 m.bevel_box('rail',(0,0.14,0.10),(0.34,0.065,0.80),0.02)
 m.cylinder('dark',(0,0.015,0.42),(0,0.015,1.52),0.067,n=10)
 m.cylinder('rail',(0,0.015,1.38),(0,0.015,1.54),0.095,n=10)
 m.cylinder('coil',(0,0.015,1.52),(0,0.015,1.55),0.065,n=10)
 m.cylinder('dark',(0,0.23,-0.17),(0,0.23,0.38),0.115,n=12)
 m.cylinder('glass',(0,0.23,0.365),(0,0.23,0.39),0.09,n=12)
 for sign in (-1,1):
  m.box('rail',(sign*0.11,0.23,0.04),(0.027,0.21,0.07))
  m.box('coil',(sign*0.125,0.04,0.73),(0.022,0.06,0.42))
  m.bevel_box('metal',(sign*0.14,-0.09,0.44),(0.075,0.16,0.36),0.02)
 m.save()

def build_shotgun():
 m=Model('gun_shotgun');palette(m)
 m.material('plate',(81,69,51));m.material('orange',(235,149,60));m.material('lamp',(255,214,123),True);m.material('titanium',(129,148,146))
 m.bevel_box('plate',(0,-0.035,0.14),(0.62,0.37,0.81),0.09)
 m.bevel_box('dark',(0,-0.28,-0.19),(0.19,0.45,0.21),0.04)
 m.bevel_box('orange',(0,0.18,0.15),(0.54,0.10,0.70),0.035)
 for x in (-0.16,0.16):
  m.cylinder('titanium',(x,0.0,0.30),(x,0.0,1.12),0.125,n=10)
  m.cylinder('dark',(x,0.0,1.10),(x,0.0,1.16),0.12,n=10)
  m.cylinder('lamp',(x,0.0,1.145),(x,0.0,1.16),0.067,n=8)
 for sign in (-1,1):m.box('orange',(sign*0.29,0.05,0.28),(0.035,0.14,0.54))
 m.bevel_box('plate',(0,-0.16,0.55),(0.38,0.18,0.30),0.035)
 m.save()

def build_minigun():
 m=Model('gun_minigun');palette(m)
 m.material('hull',(51,74,92));m.material('rotor',(134,165,176));m.material('heater',(249,182,61),True);m.material('coolant',(61,239,240),True)
 m.bevel_box('hull',(0,-0.05,0.09),(0.68,0.43,0.71),0.12)
 m.bevel_box('dark',(0,-0.32,-0.16),(0.22,0.46,0.23),0.06)
 m.cylinder('rotor',(0,0.02,0.42),(0,0.02,0.57),0.27,n=12)
 m.cylinder('hull',(0,0.02,1.17),(0,0.02,1.31),0.27,n=12)
 for i in range(6):
  angle=i*2*math.pi/6
  x=math.cos(angle)*0.18;y=0.02+math.sin(angle)*0.18
  m.cylinder('rotor',(x,y,0.47),(x,y,1.36),0.066,n=7)
  m.cylinder('heater',(x,y,1.32),(x,y,1.38),0.077,n=7)
 for side in (-1,1):
  m.bevel_box('hull',(side*0.37,-0.04,0.25),(0.21,0.28,0.57),0.06)
  m.box('coolant',(side*0.47,0.04,0.22),(0.023,0.10,0.35))
 m.box('heater',(0,0.26,-0.03),(0.22,0.044,0.31))
 m.save()

def build_marksman():
 m=Model('enemy_marksman');palette(m)
 m.material('blue',(64,160,197));m.material('cloak',(33,60,87));m.material('lens',(129,250,255),True)
 m.taper_box('cloak',0.85,1.60,(0.34,0.23),(0.42,0.26))
 m.bevel_box('blue',(0,1.37,0.20),(0.58,0.37,0.13),0.05)
 m.bevel_box('metal',(0,1.80,0.01),(0.40,0.29,0.42),0.07)
 m.cylinder('lens',(0,1.81,0.23),(0,1.81,0.29),0.115,n=12)
 for sign in (-1,1):
  m.bevel_box('cloak',(sign*0.44,1.41,-0.05),(0.31,0.30,0.40),0.06)
  m.cylinder('metal',(sign*0.47,1.16,0.02),(sign*0.49,0.71,0.12),0.11,n=7)
  m.cylinder('dark',(sign*0.21,0.88,0),(sign*0.24,0.40,0.03),0.14,n=8)
  m.bevel_box('cloak',(sign*0.24,0.17,0.15),(0.29,0.29,0.44),0.06)
  m.box('lens',(sign*0.31,1.33,0.23),(0.07,0.09,0.04))
 m.cylinder('metal',(-0.60,0.92,0.18),(0.25,0.95,0.22),0.065,n=8)
 m.cylinder('lens',(0.26,0.95,0.22),(0.33,0.95,0.22),0.064,n=8)
 m.save()

def build_gunner():
 m=Model('enemy_gunner');palette(m)
 m.material('red',(217,105,65));m.material('armor',(107,79,83));m.material('charge',(255,213,90),True)
 m.taper_box('dark',0.77,1.79,(0.54,0.39),(0.61,0.41))
 m.bevel_box('armor',(0,1.47,0.05),(1.32,0.78,0.76),0.16)
 m.bevel_box('red',(0,1.55,0.47),(0.83,0.47,0.11),0.05)
 m.box('charge',(0,1.58,0.533),(0.40,0.14,0.04))
 m.bevel_box('armor',(0,2.04,0.01),(0.69,0.35,0.57),0.08)
 m.box('charge',(0,2.1,0.31),(0.52,0.085,0.04))
 for sign in (-1,1):
  m.bevel_box('red',(sign*0.80,1.61,0.0),(0.45,0.43,0.61),0.12)
  m.cylinder('metal',(sign*0.82,1.34,0.03),(sign*0.88,0.97,0.22),0.18,n=9)
  m.cylinder('dark',(sign*0.43,0.92,0.0),(sign*0.45,0.41,0.0),0.20,n=9)
  m.bevel_box('armor',(sign*0.44,0.18,0.16),(0.41,0.30,0.64),0.08)
  m.cylinder('charge',(sign*0.89,1.00,0.25),(sign*0.89,1.00,0.37),0.105,n=8)
  m.box('red',(sign*0.69,1.58,0.34),(0.10,0.28,0.04))
 m.save()

def build_marksman_lod():
 m=Model('enemy_marksman_lod');palette(m)
 m.material('blue',(64,160,197));m.material('lens',(129,250,255),True)
 m.box('dark',(0,1.23,0),(0.7,0.71,0.43))
 m.box('blue',(0,1.39,0.25),(0.49,0.27,0.07))
 m.box('metal',(0,1.79,0.03),(0.44,0.30,0.41))
 m.box('lens',(0,1.8,0.245),(0.30,0.09,0.03))
 for sign in (-1,1):
  m.box('blue',(sign*0.42,1.27,0.04),(0.21,0.47,0.32))
  m.box('dark',(sign*0.22,0.49,0.04),(0.25,0.96,0.32))
 m.save()

def build_gunner_lod():
 m=Model('enemy_gunner_lod');palette(m)
 m.material('red',(217,105,65));m.material('charge',(255,213,90),True)
 m.box('metal',(0,1.47,0),(1.27,0.86,0.78))
 m.box('red',(0,1.5,0.43),(0.92,0.46,0.09))
 m.box('charge',(0,1.58,0.49),(0.41,0.15,0.04))
 m.box('dark',(0,2.02,0),(0.65,0.33,0.53))
 for sign in (-1,1):
  m.box('red',(sign*0.79,1.44,0),(0.37,0.54,0.52))
  m.box('charge',(sign*0.82,1.0,0.29),(0.14,0.17,0.13))
  m.box('dark',(sign*0.42,0.48,0.05),(0.36,0.90,0.41))
 m.save()

for fun in [build_sniper,build_shotgun,build_minigun,build_marksman,build_gunner,
            build_marksman_lod,build_gunner_lod]:fun()
