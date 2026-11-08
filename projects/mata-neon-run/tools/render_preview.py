"""Optional offline inspection: render the actual OBJ/MTL triangles with Pillow.

Only previews need Pillow; the Rust game uses neither Pillow nor image files.
"""
from pathlib import Path
import math,re
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[1]
SRC=ROOT/'assets/models'
DEST=ROOT/'previews';DEST.mkdir(exist_ok=True)

def vec_add(a,b):return tuple(x+y for x,y in zip(a,b))
def vec_sub(a,b):return tuple(x-y for x,y in zip(a,b))
def mul(a,f):return tuple(x*f for x in a)
def cross(a,b):return(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def dot(a,b):return sum(x*y for x,y in zip(a,b))
def norm(a):return mul(a,1/max(1e-8,math.sqrt(dot(a,a))))

def parse(name):
    verts=[];faces=[];current='default';mat={}
    for line in (SRC/(name+'.mtl')).read_text().splitlines():
        bits=line.split()
        if bits and bits[0]=='newmtl':current=bits[1];mat[current]=[(160,170,184),False]
        if bits and bits[0]=='Kd':mat[current][0]=tuple(int(min(255,max(0,round(float(v)*255)))) for v in bits[1:4])
        if bits and bits[0]=='Ke':mat[current][1]=any(float(v)>0.01 for v in bits[1:4])
    current='default'
    for line in (SRC/(name+'.obj')).read_text().splitlines():
        bits=line.split()
        if not bits:continue
        if bits[0]=='v':verts.append(tuple(map(float,bits[1:4])))
        if bits[0]=='usemtl':current=bits[1]
        if bits[0]=='f':
            indices=[int(x.split('/')[0])-1 for x in bits[1:]]
            for i in range(1,len(indices)-1):faces.append((current,[indices[0],indices[i],indices[i+1]]))
    return verts,faces,mat

def preview(name,caption,subtitle,eye,target,size=(520,550),zoom=1.0):
    img=Image.new('RGB',size,(10,18,30));d=ImageDraw.Draw(img)
    W,H=size
    for y in range(H):
        r=round(7+8*y/H);g=round(15+16*y/H);b=round(26+20*y/H)
        d.line([(0,y),(W,y)],fill=(r,g,b))
    fwd=norm(vec_sub(target,eye));right=norm(cross(fwd,(0,1,0)));up=norm(cross(right,fwd))
    focal=min(W,H)*1.5*zoom
    def proj(v):
        v=vec_sub(v,eye);distance=dot(v,fwd)
        if distance<0.2:return None
        return (W*.5+dot(v,right)*focal/distance, H*.52-dot(v,up)*focal/distance,distance)
    vertices,triangles,palette=parse(name)
    collection=[]
    sun=norm((0.5,0.88,0.36))
    for material,ids in triangles:
        pts=[vertices[i] for i in ids];coords=[proj(x) for x in pts]
        if any(v is None for v in coords):continue
        a,b,c=pts;no=norm(cross(vec_sub(b,a),vec_sub(c,a)))
        color,glow=palette.get(material,((155,167,181),False))
        light=1.12 if glow else 0.48+0.57*abs(dot(no,sun))
        color=tuple(min(255,int(x*light)) for x in color)
        collection.append((sum(p[2] for p in coords)/3,coords,color,glow))
    for _,coords,color,glow in sorted(collection,key=lambda x:-x[0]):
        p=[(int(x),int(y)) for x,y,_ in coords]
        d.polygon(p,fill=color)
        if not glow:d.line(p+[p[0]],fill=tuple(max(0,int(x*0.53)) for x in color),width=1)
    try:
        title=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',26)
        sub=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',17)
    except OSError:title=ImageFont.load_default();sub=title
    d.text((24,24),caption,font=title,fill=(215,235,247))
    d.text((25,H-50),subtitle,font=sub,fill=(118,183,202))
    out=DEST/(name+'.png');img.save(out,optimize=True)
    return img

names=[('enemy_runner','RUNNER','Fast pursuit unit',(3.0,2.8,4.3),(0,1.03,0)),('enemy_striker','STRIKER','Pincer assault unit',(3.4,2.8,4.8),(0,0.93,0)),('enemy_tank','TANK','Heavy siege walker',(3.9,3.2,5.0),(0,1.29,0)),('gun_pulse_carbine','PULSE CARBINE','Precision rapid-fire emitter',(2.7,1.4,3.9),(0,-0.02,0.39)),('gun_arc_cannon','ARC CANNON','Twin-barrel energy weapon',(2.7,1.4,3.9),(0,0.02,0.46))]
parts=[]
for name,caption,subtitle,eye,target in names:
    parts.append(preview(name,caption,subtitle,eye,target,zoom=1.45 if name.startswith("gun_") else 1.0))
contact=Image.new('RGB',(1600,1160),(7,13,23))
for im,(x,y) in zip(parts,[(20,20),(540,20),(1060,20),(280,590),(800,590)]):contact.paste(im,(x,y))
contact.save(DEST/'neon-run-models-contact-sheet.png',optimize=True)
print('Generated 5 OBJ previews + contact sheet:',DEST)
