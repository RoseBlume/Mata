"""Build original, extra-detailed 4K-view hero OBJ/MTL rigs for all five robots.

No additional Python or Rust dependencies. Retains each full-quality polygon from
make_detailed_enemies and adds authored fine hard-surface geometry. +Z faces front;
all 12 bone pivots / animation names are exactly the same as existing rigs.

Usage: python tools/make_4k_enemies.py
"""
from __future__ import annotations
import math
from make_detailed_enemies import start, BUILDERS, panel, cyl, ring

NAMES=tuple(BUILDERS)
WIDTH={'runner':0.58,'striker':0.94,'tank':1.45,'marksman':0.74,'gunner':1.13}
ARM_X={'runner':0.46,'striker':0.62,'tank':1.02,'marksman':0.52,'gunner':0.73}
HIP_X={'runner':0.24,'striker':0.37,'tank':0.49,'marksman':0.31,'gunner':0.43}


def lightpipe(m,point_a,point_b,r=0.010,mat='glow',sides=12):
    cyl(m,mat,point_a,point_b,r,sides)


def fastener(m,x,y,z,r=0.024,axis='z'):
    """Visible recessed circular head, individual screw shaft, and slot."""
    p=(x,y,z)
    if axis=='z':
        cyl(m,'carbon',(x,y,z-0.016),p,r*1.35,16)
        ring(m,'silver',(x,y,z+0.004),(0,0,1),r,r*0.62,0.017,16)
        panel(m,'shadow',x,y,z+0.021,r*1.20,r*0.24,0.009,0.003)
    else:
        cyl(m,'carbon',(x-0.016,y,z),p,r*1.35,16)
        ring(m,'silver',(x+0.004,y,z),(1,0,0),r,r*0.62,0.017,16)
        panel(m,'shadow',x+0.021,y,z,0.009,r*1.20,r*0.24,0.003)


def grille(m,x,y,z,w,h,rows=11):
    # Deep surround / evenly spaced ventilation fins, at real 3D depth.
    panel(m,'shadow',x,y,z,w,h,0.026,0.017)
    inner=h-0.045
    for i in range(rows):
        yy=y-inner/2+inner*(i+0.5)/rows
        panel(m,'carbon',x,yy,z+0.020,w-0.06,0.018,0.012,0.005)
        panel(m,'silver',x,yy+0.004,z+0.029,w-0.08,0.004,0.008,0.002)
    for sx in (-1,1):
        fastener(m,x+sx*(w*0.5-0.028),y,z+0.026,0.011)


def coil(m,a,b,r=0.12,turns=7,steps=15):
    # Ribbed high-frequency details for exposed actuators (short swept tube
    # segments; each segment is genuinely round instead of a pixel line).
    dx=tuple(b[i]-a[i] for i in range(3))
    for k in range(turns):
        t=(k+.35)/turns
        center=tuple(a[i]+dx[i]*t for i in range(3))
        ring(m,'silver',center,dx,r,r*.77,0.013,steps)


def add_body(m,kind):
    width=WIDTH[kind]
    mid=1.64 if kind=='tank' else 1.30
    height=0.95 if kind=='tank' else {'runner':0.57,'striker':0.70,'marksman':0.65,'gunner':0.76}[kind]
    with m.bone('body'):
        # A layered forward chest assembly, exposed throat frame, trim strips.
        panel(m,'carbon',0,mid+0.08,0.437,width*.32,height*.24,0.030,0.022)
        panel(m,'metal',0,mid+0.08,0.468,width*.25,height*.18,0.031,0.019)
        panel(m,'glow',0,mid+0.08,0.491,width*.12,height*.08,0.024,0.012)
        for s in (-1,1):
            side=s*width*0.33
            # Fine tiered armour trim (preserves angular high-detail silhouette).
            for i in range(8):
                yy=mid-height*.31+i*height*.085
                x=side+s*(0.009 if i%2 else -0.009)
                panel(m,'metal',x,yy,0.461,width*.19,height*.052,0.027,0.010)
                panel(m,'edge',x,yy+height*.012,0.481,width*.16,height*.008,0.012,0.004)
            lightpipe(m,(side,mid-height*.24,0.461),(side,mid+height*.31,0.466),0.009,'glow',12)
            for jy in (-0.23,0.26):
                fastener(m,side,mid+jy*height,0.484,0.023)
            # Back service manifolds and braided power feeds follow the torso.
            panel(m,'metal',s*(width*.36),mid+0.13,-0.35,width*.19,height*.51,0.095,0.022)
            for index in range(8):
                y=mid-height*.17+index*height*.055
                panel(m,'silver',s*(width*.36),y,-0.405,width*.14,0.014,0.014,0.005)
            for j in range(2):
                x=s*(width*.42+0.03*j)
                lightpipe(m,(x,mid+height*.32,-0.18),(x,mid-height*.28,-0.20),0.019,'cable',12)
                coil(m,(x,mid-height*.25,-.20),(x,mid+height*.26,-.20),0.027,6,12)
        # Additional front-facing chest status rails and spines.
        for i in range(10):
            x=(i-4.5)*width*.057
            panel(m,'edge',x,mid-height*.37,.433,width*.031,0.024,0.019,0.005)
        for i in range(6):
            y=mid+height*.31+i*0.027
            panel(m,'carbon',0,y,-.325,width*.50,0.010,0.026,0.003)
        # Belt and waist yoke.
        for s in (-1,1):
            ring(m,'silver',(s*width*.28,1.01,0.26),(0,0,1),0.087,0.057,0.050,24)
            for k in range(5):
                x=s*width*(0.10+0.046*k)
                panel(m,'shell_hi',x,1.06,0.314,width*.035,0.083,0.042,0.009)


def add_head(m,kind):
    head_y=2.28 if kind=='tank' else 1.92 if kind=='gunner' else 1.86
    # The head is independently articulated; detail must use the same bone.
    with m.bone('head'):
        for s in (-1,1):
            # Ear motors, concentric bearing races and inset outer screws.
            x=s*({'tank':0.56,'gunner':0.43}.get(kind,0.29))
            ring(m,'joint',(x,head_y,0.0),(1,0,0),0.13,0.076,0.052,32)
            ring(m,'silver',(x+s*0.037,head_y,0.0),(1,0,0),0.115,0.090,0.016,32)
            ring(m,'metal',(x+s*0.062,head_y,0.0),(1,0,0),0.071,0.025,0.019,24)
            for k in range(3):
                y=head_y-0.12+k*0.095
                panel(m,'metal',x*0.82,y,0.38,0.095,0.030,0.023,0.007)
            fastener(m,x,head_y-0.18,0.34,0.016)
        # Optic with multiple concentric, true 3D glass/sensor shells.
        face_z=0.47 if kind!='tank' else 0.54
        ring(m,'shadow',(0,head_y+0.005,face_z),(0,0,1),0.19,0.115,0.045,40)
        ring(m,'silver',(0,head_y+0.005,face_z+0.023),(0,0,1),0.171,0.130,0.018,40)
        ring(m,'glow',(0,head_y+0.005,face_z+0.050),(0,0,1),0.134,0.102,0.013,40)
        ring(m,'carbon',(0,head_y+0.005,face_z+0.075),(0,0,1),0.100,0.024,0.024,32)
        # Reflection arcs; no textures required.
        for k in range(10):
            x=-0.115+k*0.025
            panel(m,'silver',x,head_y+0.20,0.351,0.016,0.029,0.025,0.004)
        for s in (-1,1):
            for i in range(6):
                y=head_y-0.17+i*0.045
                panel(m,'shadow',s*0.18,y,0.41,0.052,0.013,0.016,0.004)


def add_leg(m,kind,sign):
    side='l' if sign<0 else 'r'
    hx=sign*HIP_X[kind]
    # Fine plating and structural actuators move with their own bones.
    with m.bone('thigh_'+side):
        ring(m,'metal',(hx,1.035,0.03),(1,0,0),0.156,0.112,0.050,24)
        ring(m,'silver',(hx,1.037,0.04),(1,0,0),0.115,0.068,0.020,24)
        for i in range(10):
            y=0.61+i*0.043
            panel(m,'metal',hx,y,0.287,0.24,0.028,0.023,0.007)
            panel(m,'shell_hi',hx,y+0.008,0.305,0.16,0.008,0.012,0.003)
        for column in (-1,1):
            xx=hx+column*0.13
            lightpipe(m,(xx,0.66,0.09),(xx,1.03,0.06),0.022,'silver',12)
            for i in range(6):
                fastener(m,xx,0.70+i*0.055,0.105,0.012)
    with m.bone('shin_'+side):
        ring(m,'joint',(hx+sign*.065,0.56,0.17),(1,0,0),0.137,0.071,0.054,24)
        for i in range(11):
            y=0.15+i*0.033
            panel(m,'shadow',hx,y,0.307,0.185,0.017,0.019,0.006)
            panel(m,'silver',hx,y+0.005,0.325,0.14,0.006,0.011,0.002)
        for s in (-1,1):
            xx=hx+s*0.095
            lightpipe(m,(xx,0.16,-0.16),(xx,0.52,-0.16),0.028,'metal',16)
            coil(m,(xx,0.20,-0.16),(xx,0.49,-0.16),0.047,8,16)
    with m.bone('foot_'+side):
        for i in range(6):
            zz=0.16+i*0.058
            panel(m,'carbon',hx,0.100,zz,0.20,0.024,0.022,0.004)
            panel(m,'silver',hx,0.113,zz,0.19,0.008,0.022,0.003)
        for s in (-1,1):
            for z in (0.15,0.33):
                fastener(m,hx+s*.125,0.155,z,0.012)


def add_arm(m,kind,sign):
    side='l' if sign<0 else 'r'
    x=sign*ARM_X[kind]
    sy=1.95 if kind=='tank' else 1.48
    elbow=1.30 if kind=='tank' else 0.99
    with m.bone('upper_arm_'+side):
        ring(m,'metal',(x,sy,0.25),(0,0,1),0.145,0.084,0.038,30)
        ring(m,'glow',(x,sy,0.275),(0,0,1),0.107,0.088,0.015,30)
        for i in range(10):
            yy=elbow+0.045+i*(sy-elbow-.08)/9
            panel(m,'metal',x,yy,0.273,0.21,0.021,0.023,0.005)
            panel(m,'edge',x,yy+0.006,0.286,0.12,0.006,0.009,0.003)
        for sx in (-1,1):
            xx=x+sx*0.15
            cyl(m,'cable',(xx,sy-.09,-0.13),(xx,elbow+.09,-.13),0.024,14)
            ring(m,'silver',(xx,elbow+.07,-.13),(0,1,0),0.035,0.024,0.018,16)
    with m.bone('forearm_'+side):
        ring(m,'joint',(x+sign*.06,elbow,.13),(1,0,0),0.117,0.064,0.043,24)
        for i in range(8):
            y=elbow-0.08-i*0.040
            panel(m,'metal',x+sign*.07,y,0.330,0.242,0.019,0.023,0.007)
            panel(m,'shell_hi',x+sign*.07,y+0.004,0.346,0.15,0.004,0.010,0.001)
        for i in range(6):
            fastener(m,x+sign*.085,elbow-.10-i*.060,0.358,0.011)


def characteristic_details(m,kind):
    if kind=='runner':
        with m.bone('body'):
            for s in (-1,1):
                x=s*0.36
                for i in range(8):
                    z=-0.18+i*0.056
                    panel(m,'shell_hi',x,1.45,z,0.075,0.035,0.046,0.009)
                    fastener(m,x,1.49,z+.03,0.011)
    elif kind=='striker':
        for s in (-1,1):
            side='l' if s<0 else 'r'
            with m.bone('forearm_'+side):
                for i in range(6):
                    z=.46+i*.05
                    cyl(m,'silver',(s*.78,.64,z),(s*.78,.79,z+.085),.023,14)
                    panel(m,'warning',s*.80,.78,z+.035,.055,.035,.034,0.009)
    elif kind=='tank':
        with m.bone('body'):
            grille(m,0,1.68,0.558,0.67,0.43,16)
            for s in (-1,1):
                for i in range(9):
                    x=s*(.55+i*.023)
                    panel(m,'metal',x,2.03,-.48,0.021,0.24,0.075,0.005)
    elif kind=='marksman':
        with m.bone('body'):
            ring(m,'metal',(.38,1.82,.84),(0,0,1),0.22,0.155,0.080,40)
            ring(m,'silver',(.38,1.82,.89),(0,0,1),0.181,0.134,0.028,40)
            ring(m,'glow',(.38,1.82,.92),(0,0,1),0.140,0.120,0.012,40)
            for i in range(10):
                zz=0.11+i*0.06
                panel(m,'metal',.53,1.47,zz,.16,0.024,.028,0.005)
    elif kind=='gunner':
        for s in (-1,1):
            side='l' if s<0 else 'r'
            with m.bone('forearm_'+side):
                x=s*.91
                for i in range(10):
                    zz=.39+i*.068
                    ring(m,'metal',(x,1.13,zz),(0,0,1),.207,.169,.025,24)
                    for xx in (-1,1):
                        fastener(m,x+xx*.13,1.13,zz+.015,.011)
                ring(m,'glow',(x,1.13,1.113),(0,0,1),.14,.085,.016,32)


def build(name):
    m=start(name,low=False)
    BUILDERS[name](m,False)
    original_tris=sum(len(f)-2 for _,f in m.faces)
    # New version of the existing authorable rig: its base geometry is intact.
    add_body(m,name)
    add_head(m,name)
    for sign in (-1,1):
        add_leg(m,name,sign)
        add_arm(m,name,sign)
    characteristic_details(m,name)
    m.name='enemy_'+name+'_4k'
    m.save()
    total=m.save_rig(name)
    assert total==sum(len(f)-2 for _,f in m.faces)
    assert total>original_tris*2.0,(name,original_tris,total)
    # The hero tier intentionally costs more than current full. The renderer
    # uses it only for genuinely large on-screen robots.
    assert total<80000,(name,'excessive hero mesh',total)
    return original_tris,total,len(m.v)


if __name__=='__main__':
    for name in NAMES:
        base,tris,verts=build(name)
        print(f'{name}: full={base:,} hero4k={tris:,} triangles, {verts:,} vertices (+{tris-base:,} authored detail triangles)')
