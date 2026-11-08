"""Generate five high-detail procedural enemy OBJ/MTL meshes and five LODs.

Uses only Python's standard library. All geometry is original, fictional sci-fi
robot armour. +Z is forward and Y is up; the actor pivot is at the feet.
Run: python tools/make_detailed_enemies.py
"""
from pathlib import Path
import math
from collections import OrderedDict
from contextlib import contextmanager
OUT=Path(__file__).resolve().parents[1]/'assets/models'
OUT.mkdir(exist_ok=True,parents=True)

class Model:
 def __init__(self,name):
  self.name=name;self.v=[];self.faces=[];self.face_parts=[];self.active_part="body";self.materials=OrderedDict()
 @contextmanager
 def bone(self,part):
  old=self.active_part;self.active_part=part
  try:yield
  finally:self.active_part=old
 def material(self,name,rgb,glow=False):
  self.materials[name]=(rgb,glow)
 def vert(self,v):
  self.v.append(tuple(v));return len(self.v)
 def face(self,mat,points):
  self.faces.append((mat,tuple(points)))
  self.face_parts.append(self.active_part)
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
 def save_rig(self,kind):
  """Preserve EVERY original polygon, rebased around articulated joints.

  Part OBJ geometry is full-resolution: no remeshing, skin weight artifacts,
  baked frames, atlas loss or geometric decimation. Part face sums must exactly
  match each corresponding full static model including the medium LOD.
  """
  import os
  root=OUT/'rig';root.mkdir(exist_ok=True)
  expected={
   'body','head',
   'thigh_l','shin_l','foot_l','thigh_r','shin_r','foot_r',
   'upper_arm_l','forearm_l','upper_arm_r','forearm_r',
  }
  assert set(self.face_parts)==expected,(self.name,set(self.face_parts)^expected)
  hip_x={'runner':.24,'striker':.37,'tank':.49,'marksman':.31,'gunner':.43}[kind]
  arm_x={'runner':.46,'striker':.62,'tank':1.02,'marksman':.52,'gunner':.73}[kind]
  arm_y=1.95 if kind=='tank' else 1.48
  head_y=2.28 if kind=='tank' else 1.86 if kind!='gunner' else 1.92
  pivots={'body':(0,0,0),'head':(0,head_y,0)}
  for side,sign in (('l',-1),('r',1)):
   x=sign*hip_x
   a=arm_x*sign
   pivots.update({
     'thigh_'+side:(x,1.05,-.02),
     'shin_'+side:(x+sign*.065,.56,.17),
     'foot_'+side:(x+sign*.02,.13,.12),
     'upper_arm_'+side:(a,arm_y,0.02),
     'forearm_'+side:(sign*(arm_x+.06),.99 if kind!='tank' else 1.30,.13),
   })
  total_tri=0
  for part in sorted(expected):
   indices=[i for i,assigned in enumerate(self.face_parts) if assigned==part]
   assert indices,(self.name,part)
   face_list=[self.faces[i] for i in indices]
   used=sorted(set(v for _,poly in face_list for v in poly))
   remap={k:i+1 for i,k in enumerate(used)}
   px,py,pz=pivots[part]
   output=[f'# Exact original {self.name} triangles; {part} local joint coordinates',
           f'# pivot {px:.6f} {py:.6f} {pz:.6f}',
           f'mtllib ../{self.name}.mtl','']
   for old in used:
    x,y,z=self.v[old-1]
    output.append('v %.6f %.6f %.6f'%(x-px,y-py,z-pz))
   # Material batch count identical or less per part, maintained after partition.
   for mat in self.materials:
    subset=[poly for material,poly in face_list if material==mat]
    if not subset:continue
    output.append('usemtl '+mat)
    output.extend('f '+' '.join(str(remap[v]) for v in poly) for poly in subset)
   (root/(self.name+'_'+part+'.obj')).write_text('\n'.join(output)+'\n')
   total_tri+=sum(len(poly)-2 for _,poly in face_list)
  assert total_tri==sum(len(poly)-2 for _,poly in self.faces),self.name
  return total_tri

 def save(self):
  obj=['# MATA NEON RUN - Detailed hard-surface robotic character',f'# {self.name}',f'mtllib {self.name}.mtl','']
  for v in self.v:obj.append('v '+' '.join(f'{n:.6f}' for n in v))
  # Group polygons by their material. This minimizes material changes and
  # the number of draw batches the embedded Rust OBJ loader must produce.
  for mat in self.materials:
   polys=[face for face_mat,face in self.faces if face_mat==mat]
   if not polys:continue
   obj.append(f'usemtl {mat}')
   for face in polys:obj.append('f '+' '.join(str(x) for x in face))
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


# Common armour grammar: industrial metals, illuminated optics, exposed mechanics.
# Distant LOD keeps readable silhouettes but omits bolts, pistons and fine grooves.
PALETTES={
    'runner':{
        'shell':(215,122,42),'shell_hi':(249,168,80),'shadow':(85,53,37),
        'glow':(77,238,246),'warning':(255,200,96)},
    'striker':{
        'shell':(178,48,112),'shell_hi':(252,95,165),'shadow':(82,45,104),
        'glow':(255,96,204),'warning':(120,244,242)},
    'tank':{
        'shell':(121,92,185),'shell_hi':(170,133,218),'shadow':(71,64,112),
        'glow':(216,119,252),'warning':(100,239,242)},
    'marksman':{
        'shell':(49,119,143),'shell_hi':(109,191,205),'shadow':(37,76,109),
        'glow':(106,239,240),'warning':(255,190,101)},
    'gunner':{
        'shell':(141,124,67),'shell_hi':(199,169,96),'shadow':(83,85,59),
        'glow':(255,175,80),'warning':(103,248,198)},
}

def start(name,low=False):
    m=Model('enemy_'+name+('_lod' if low else ''))
    for mat,col in {
        'carbon':(16,25,36), 'joint':(33,46,57),'metal':(54,73,83),
        'silver':(141,164,171),'edge':(200,218,223),'rubber':(23,32,39),
        'cable':(57,73,89),**PALETTES[name],
    }.items():m.material(mat,col,mat in ('glow','warning'))
    return m

# Fine geometry helpers. These are all just OBJ triangles/quads, no textures.
def panel(m,mat,x,y,z,sx,sy,sz,bevel=0.035):
    m.bevel_box(mat,(x,y,z),(sx,sy,sz),bevel)

def cyl(m,mat,a,b,rad=0.07,n=8,tip=None):
    m.cylinder(mat,a,b,rad,rad if tip is None else tip,n=n)

def ring(m,material,center,axis,r_outer,r_inner,depth=0.08,segments=12):
    """An open, annular ring with visible bevel-like inner walls."""
    cx,cy,cz=center
    ax=list(axis);mag=math.sqrt(sum(v*v for v in ax));ax=[v/mag for v in ax]
    reference=[0,1,0] if abs(ax[1])<0.88 else [1,0,0]
    u=[ax[1]*reference[2]-ax[2]*reference[1],ax[2]*reference[0]-ax[0]*reference[2],ax[0]*reference[1]-ax[1]*reference[0]]
    mgu=math.sqrt(sum(x*x for x in u));u=[x/mgu for x in u]
    v=[ax[1]*u[2]-ax[2]*u[1],ax[2]*u[0]-ax[0]*u[2],ax[0]*u[1]-ax[1]*u[0]]
    rr=[]
    for axial,radius in [(-depth/2,r_outer),(depth/2,r_outer),(-depth/2,r_inner),(depth/2,r_inner)]:
        rr.append([m.vert((cx+ax[0]*axial+radius*(u[0]*math.cos(i*2*math.pi/segments)+v[0]*math.sin(i*2*math.pi/segments)),
                          cy+ax[1]*axial+radius*(u[1]*math.cos(i*2*math.pi/segments)+v[1]*math.sin(i*2*math.pi/segments)),
                          cz+ax[2]*axial+radius*(u[2]*math.cos(i*2*math.pi/segments)+v[2]*math.sin(i*2*math.pi/segments)))) for i in range(segments)])
    for i in range(segments):
        j=(i+1)%segments
        m.face(material,[rr[0][i],rr[0][j],rr[1][j],rr[1][i]])
        m.face(material,[rr[2][i],rr[3][i],rr[3][j],rr[2][j]])
        m.face(material,[rr[0][i],rr[2][i],rr[2][j],rr[0][j]])
        m.face(material,[rr[1][j],rr[3][j],rr[3][i],rr[1][i]])

def studs(m,x,y,z,dx=0.1,dy=0.1,rows=2,cols=3):
    for row in range(rows):
        for col in range(cols):
            m.box('silver',(x+col*dx,y+row*dy,z),(0.024,0.026,0.021))

def back_spine(m,high=2.0,low=False):
    panel(m,'carbon',0.0,1.48,-0.32,0.18,0.75,0.16)
    if not low:
        for i in range(5):
            panel(m,'silver',0.0,1.16+i*.145,-0.421,0.17,0.04,0.035,0.011)
    panel(m,'metal',0.0,high,-0.30,0.26,0.18,0.13)

def leg(m,sign,build,low=False):
    side='l' if sign<0 else 'r'
    x=sign*(0.24 if build=='runner' else 0.37 if build=='striker' else 0.49 if build=='tank' else 0.31 if build=='marksman' else 0.43)
    a=(x,1.05,-0.02);knee=(x+sign*0.065,0.56,0.17);ankle=(x+sign*0.02,0.13,0.12)
    r=0.11 if build=='runner' else 0.15 if build in ('marksman','striker') else 0.20
    with m.bone('thigh_'+side):
        cyl(m,'joint',a,knee,r,8)
        panel(m,'shell',x,0.83,0.09,r*2.4,0.32,0.25)
        if not low:
            cyl(m,'silver',(x-sign*r*.7,0.91,-0.13),(x-sign*r*.7,0.56,-0.12),0.035,8)
            cyl(m,'joint',(x,0.83,-0.21),(x,0.59,-0.19),0.047,8)
    with m.bone('shin_'+side):
        cyl(m,'metal',knee,ankle,r*.83,8)
        panel(m,'shell_hi',knee[0],knee[1]+0.02,knee[2]+0.14,r*2.2,0.22,0.19)
        panel(m,'carbon',x,0.26,0.11,r*2.25,0.32,0.23)
        if not low:
            cyl(m,'silver',(x-sign*r*.7,0.56,-0.12),(x-sign*r*.7,0.43,-0.12),0.035,8)
            ring(m,'silver',knee,(1,0,0),r*.78,r*.39,0.05,10)
            for y in [0.31,0.38,0.45]:
                panel(m,'shadow',x,y,0.246,r*1.6,0.025,0.026,0.008)
    with m.bone('foot_'+side):
        panel(m,'rubber',ankle[0],0.09,0.29,r*2.8,0.17,0.53)
        if not low:
            for toe in (-1,1):
                panel(m,'metal',ankle[0]+toe*r*.60,0.09,0.44,r*.70,0.09,0.26,0.013)

def head(m,kind,low=False):
 with m.bone("head"):
     y=1.86 if kind in ('runner','striker','marksman') else 2.28 if kind=='tank' else 1.92
     width=0.43 if kind in ('runner','marksman') else 0.55 if kind!='tank' else 0.72
     panel(m,'joint',0,y-0.16,0.0,width*.62,0.29,0.38)
     panel(m,'shell',0,y,0.045,width,0.33,0.37)
     panel(m,'shadow',0,y-0.03,0.25,width*0.86,0.18,0.07)
     panel(m,'glow',0,y-0.025,0.291,width*0.75,0.087,0.04)
     if low:return
     panel(m,'shell_hi',0,y+0.19,0.065,width*0.82,0.10,0.36)
     ring(m,'metal',(0,y+.06,0.32),(0,0,1),0.085,0.052,0.05,10)
     ring(m,'glow',(0,y+.06,0.35),(0,0,1),0.065,0.036,0.022,10)
     for sign in (-1,1):
         panel(m,'silver',sign*width*.40,y,0.28,0.05,0.13,0.11)
         cyl(m,'cable',(sign*width*.45,y-.13,-0.03),(sign*width*.58,y-.24,-.20),0.031,8)
     if kind=='marksman':
         cyl(m,'joint',(.09,y+.24,-.03),(.11,y+.60,.04),.055,10)
         ring(m,'glow',(.11,y+.60,.04),(0,1,0),.082,.044,.06,12)
         panel(m,'shell_hi',-.13,y+.21,-.02,.18,.09,.32)
     elif kind=='runner':
         for sign in (-1,1):
             m.arrow('shell_hi',(sign*.26,y+.22,-.16),(.12,.22,.22))
 

def torso(m,kind,low=False):
    wide={'runner':0.58,'striker':0.94,'tank':1.45,'marksman':0.74,'gunner':1.13}[kind]
    h={'runner':0.57,'striker':0.70,'tank':0.95,'marksman':0.65,'gunner':0.76}[kind]
    centre=1.30 if kind!='tank' else 1.64
    panel(m,'joint',0,centre,0.0,wide*.95,h,0.58,0.08)
    panel(m,'shell',0,centre+0.03,0.26,wide*.84,h*.76,0.16,0.06)
    panel(m,'shadow',0,centre+.13,.355,wide*.67,h*.50,0.075,0.02)
    panel(m,'glow',0,centre+.10,.41,wide*.35,h*.16,0.06,0.01)
    panel(m,'metal',0,centre-0.33,0.0,wide*.70,0.18,0.42)
    if low:return
    back_spine(m,centre+0.32,low)
    for sign in (-1,1):
        panel(m,'shell_hi',sign*wide*.30,centre+.15,.35,wide*.30,h*.41,.07)
        cyl(m,'joint',(sign*wide*.41,centre-.28,-.06),(sign*wide*.43,centre+.27,-.08),.051,8)
        ring(m,'silver',(sign*wide*.33,centre-0.20,0.355),(0,0,1),.079,.046,.034,8)
        for k in range(3):
            panel(m,'silver',sign*(wide*.38),centre-.14+k*.118,.419,.047,.022,.035,.01)
    for i in range(5):
        panel(m,'shell_hi',-.20+i*.10,centre-h*.35,.38,.052,.05,.055,.014)

def shoulder(m,x,y,wide,low=False):
    panel(m,'joint',x,y,0.02,wide*0.80,0.33,0.42)
    panel(m,'shell',x,y+0.085,-0.01,wide,0.29,0.47,0.065)
    if low:return
    panel(m,'shell_hi',x,y+0.23,0.06,wide*.84,0.095,0.45,0.018)
    panel(m,'shadow',x,y+.04,0.246,wide*.76,0.15,0.055,0.02)
    ring(m,'metal',(x,y,.28),(0,0,1),.108,.059,.055,12)
    studs(m,x-wide*.27,y+.23,.085,dx=.083,dy=.06,rows=1,cols=3)

def simple_arm(m,sign,kind,low=False):
    side='l' if sign<0 else 'r'
    a=0.46 if kind=='runner' else 0.62 if kind=='striker' else 1.02 if kind=='tank' else 0.52 if kind=='marksman' else 0.73
    sy=1.48 if kind!='tank' else 1.95
    elbow=(sign*(a+.06),0.99 if kind!='tank' else 1.30,0.13)
    end=(sign*(a+.06),0.76 if kind!='tank' else 0.95,0.13)
    r=0.14 if kind!='tank' else 0.22
    with m.bone('upper_arm_'+side):
        shoulder(m,sign*a,sy,.32 if kind in ('runner','marksman') else .43 if kind=='striker' else .62,low)
        cyl(m,'joint',(sign*a,sy-.12,0),elbow,r,8)
        if not low:
            cyl(m,'silver',(sign*(a+.23),sy-.20,-.12),(sign*(a+.25),elbow[1],-.10),.041,8)
    with m.bone('forearm_'+side):
        cyl(m,'joint',elbow,end,r*.94,8)
        panel(m,'shell',sign*(a+.07),(elbow[1]+end[1])*.50,.10,
              .26 if kind!='tank' else .42,.42,.37)
        if not low:
            panel(m,'glow',sign*(a+.07),1.08,.31,.14,.05,.04,.017)

def runner(m,low):
    torso(m,'runner',low);head(m,'runner',low)
    for side in (-1,1):
        leg(m,side,'runner',low)
        simple_arm(m,side,'runner',low)
        side_name='l' if side<0 else 'r'
        with m.bone('forearm_'+side_name):
            panel(m,'shell',side*.56,.79,.27,.24,.20,.20)
            m.arrow('shell_hi',(side*.63,.68,.38),(.09,.23,.26))
            if not low:cyl(m,'silver',(side*.55,1.11,-.14),(side*.65,.83,-.22),.052,8)
        with m.bone('upper_arm_'+side_name):
            panel(m,'shell_hi',side*.41,1.61,-.16,.15,.24,.45)
        if not low:
            with m.bone('body'):
                ring(m,'glow',(side*.29,1.42,-.43),(0,0,1),.11,.055,.10,12)
                for i in range(3):
                    panel(m,'edge',side*.27,1.22+i*.09,.445,.066,.019,.022,.006)
    panel(m,'shell_hi',0,1.0,.11,.43,.21,.44)


def striker(m,low):
    torso(m,'striker',low);head(m,'striker',low)
    for side in (-1,1):
        leg(m,side,'striker',low)
        simple_arm(m,side,'striker',low)
        x=side*.82
        side_name='l' if side<0 else 'r'
        with m.bone('forearm_'+side_name):
            panel(m,'shadow',x,.87,.19,.32,.48,.38)
            m.arrow('shell_hi',(x+side*.07,.58,.40),(.17,.43,.36))
            m.arrow('shell',(x-side*.14,.60,.36),(.12,.35,.37))
            if not low:cyl(m,'glow',(x,.88,.38),(x,.61,.47),.045,10)
        with m.bone('upper_arm_'+side_name):
            m.arrow('shell_hi',(side*.73,1.80,-.20),(.20,.35,.27))
            if not low:ring(m,'silver',(x,1.18,-.26),(0,0,1),.12,.07,.08,10)
        if not low:
            with m.bone('body'):
                for k in range(3):panel(m,'shadow',side*.49,1.21+k*.12,-.35,.14,.05,.10)
    m.octa('glow',(0,1.39,.45),(.16,.14,.10))


def tank(m,low):
    torso(m,'tank',low);head(m,'tank',low)
    for side in (-1,1):
        leg(m,side,'tank',low);simple_arm(m,side,'tank',low)
        side_name='l' if side<0 else 'r'
        with m.bone('upper_arm_'+side_name):
            panel(m,'shell_hi',side*.95,1.90,0.25,.55,.74,.27)
            panel(m,'shadow',side*.98,1.82,.40,.37,.44,.075)
            panel(m,'glow',side*.98,1.82,.445,.13,.20,.04)
            if not low:
                ring(m,'warning',(side*.99,1.87,.455),(0,0,1),.13,.07,.04,12)
                for k in range(4):panel(m,'silver',side*.53,1.92+k*.13,-.46,.32,.035,.07)
        with m.bone('body'):
            cyl(m,'metal',(side*.62,2.31,-.34),(side*.64,2.42,-.56),.19,10)
            cyl(m,'metal',(side*.64,2.42,-.56),(side*.64,2.42,-.87),.16,10)
            panel(m,'rubber',side*.64,2.42,-.88,.38,.28,.09)
        if not low:
            with m.bone('shin_'+side_name):
                cyl(m,'silver',(side*.40,.56,-.25),(side*.42,.20,-.25),.055,8)
            with m.bone('thigh_'+side_name):
                cyl(m,'silver',(side*.40,.96,-.25),(side*.40,.56,-.25),.055,8)
    m.octa('glow',(0,1.70,.48),(.25,.24,.15))
    panel(m,'shell',0,1.04,.26,.75,.25,.44)


def marksman(m,low):
    torso(m,'marksman',low);head(m,'marksman',low)
    for side in (-1,1):
        leg(m,side,'marksman',low)
        simple_arm(m,side,'marksman',low)
        side_name='l' if side<0 else 'r'
        with m.bone('upper_arm_'+side_name):
            panel(m,'shell_hi',side*.52,1.55,-.20,.16,.26,.47)
        if not low:
            with m.bone('body'):
                for idx in range(5):panel(m,'shadow',side*.46,.84+idx*.115,.17,.18,.029,.14)
    # Asymmetric optical targeting array, distinctly marksman rather than infantry.
    cyl(m,'carbon',(.38,1.82,.08),(.38,1.82,.79),.09,12)
    cyl(m,'shell',(.38,1.82,.18),(.38,1.82,.61),.135,12)
    ring(m,'glow',(.38,1.82,.79),(0,0,1),.16,.10,.09,14 if not low else 8)
    panel(m,'shadow',.38,1.82,.86,.18,.18,.045)
    cyl(m,'metal',(.55,1.37,-.1),(.55,1.37,.76),.083,10)
    if not low:
        for i in range(7):panel(m,'silver',.53,1.47,.05+i*.104,.10,.032,.042,.009)
        cyl(m,'warning',(.50,1.28,.73),(.50,1.28,.83),.08,10)
        cyl(m,'silver',(-.20,2.07,-.22),(-.33,2.31,-.04),.04,8)
    panel(m,'glow',.38,1.82,.88,.11,.11,.035)


def gunner(m,low):
    torso(m,'gunner',low);head(m,'gunner',low)
    for side in (-1,1):
        leg(m,side,'gunner',low)
        simple_arm(m,side,'gunner',low)
        side_name='l' if side<0 else 'r'
        with m.bone('forearm_'+side_name):
            panel(m,'shell_hi',side*.77,1.16,.20,.44,.46,.39)
            # Side-mounted gun barrels now follow the elbow/forearm, too.
            panel(m,'shadow',side*.91,1.07,.33,.34,.32,.55)
            cyl(m,'metal',(side*.91,1.13,.31),(side*.91,1.13,.99),.18,12 if not low else 8)
            ring(m,'glow',(side*.91,1.13,1.01),(0,0,1),.19,.102,.11,12 if not low else 8)
            cyl(m,'carbon',(side*.91,1.13,1.01),(side*.91,1.13,1.12),.080,10)
            if not low:
                for idx in range(8):
                    angle=idx*math.tau/8
                    x=side*.91+.14*math.cos(angle);y=1.13+.14*math.sin(angle)
                    cyl(m,'silver',(x,y,.41),(x,y,.78),.028,6)
                studs(m,side*.67,1.51,.436,dx=.076,dy=.065,cols=4,rows=2)
        if not low:
            with m.bone('body'):
                for j in range(3):panel(m,'shell',side*.48,1.46+j*.14,-.36,.35,.06,.12)
    panel(m,'warning',0,1.30,.43,.21,.10,.06)

BUILDERS={'runner':runner,'striker':striker,'tank':tank,'marksman':marksman,'gunner':gunner}
if __name__=='__main__':
    for name,build in BUILDERS.items():
        for low in (False,True):
            m=start(name,low)
            build(m,low)
            m.save()
            m.save_rig(name)
            tri=sum(max(0,len(face)-2) for _,face in m.faces)
            assert tri<6500 if not low else tri<2200,(name,low,tri)
            print('  ->',tri,'triangles,',len(m.materials),'material slots')
