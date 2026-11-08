from pathlib import Path
root=Path(__file__).resolve().parents[1]/'assets/models'
root.mkdir(parents=True,exist_ok=True)
palette={'dark':(.08,.12,.18),'metal':(.24,.31,.40),'edge':(.48,.60,.70),'screen':(.06,.53,.73),'cyan':(.28,.91,1.0),'blue':(.16,.39,.95),'amber':(1.0,.64,.22),'red':(.95,.19,.27),'violet':(.67,.35,.96),'green':(.34,.95,.69),'glass':(.09,.23,.32),'steel':(.35,.38,.46),'rubber':(.09,.1,.14),'paper':(.75,.76,.64)}
class Mesh:
 def __init__(self):self.v=[];self.faces=[]
 def box(self,x,y,z,w,h,d,mat):
  i=len(self.v)+1;self.v.extend([(x,y,z),(x+w,y,z),(x+w,y+h,z),(x,y+h,z),(x,y,z+d),(x+w,y,z+d),(x+w,y+h,z+d),(x,y+h,z+d)])
  for face in [(1,4,3,2),(5,6,7,8),(1,5,8,4),(2,3,7,6),(4,8,7,3),(1,2,6,5)]:self.faces.append((mat,tuple(i+z-1 for z in face)))
 def save(self,name):
  mtl=['# Hand-built MATA low-poly environmental prop']
  for mat,(r,g,b) in palette.items():
   mtl.extend([f'newmtl {mat}',f'Kd {r:.5f} {g:.5f} {b:.5f}'])
   if mat in ['cyan','blue','amber','red','violet','green','screen']:mtl.append('Ke 0.72 0.72 0.72')
   mtl.append('')
  (root/f'{name}.mtl').write_text('\n'.join(mtl))
  lines=[f'# {name}: original procedural 3D asset',f'mtllib {name}.mtl']
  lines.extend(f'v {x:.5f} {y:.5f} {z:.5f}' for x,y,z in self.v)
  old=None
  for mat,face in self.faces:
   if mat!=old:lines.append(f'usemtl {mat}');old=mat
   lines.append('f '+' '.join(map(str,(face[0],face[1],face[2]))))
   lines.append('f '+' '.join(map(str,(face[0],face[2],face[3]))))
  (root/f'{name}.obj').write_text('\n'.join(lines)+'\n')
  print(name,len(self.faces)*2,'triangles')

def a(kind):
 m=Mesh(); b=m.box
 if kind=='ceiling_light':
  b(-.68,0,-.25,1.36,.09,.5,'metal');b(-.56,-.025,-.16,1.12,.03,.32,'cyan');b(-.72,.05,-.29,.2,.13,.58,'dark');b(.52,.05,-.29,.2,.13,.58,'dark')
  for x in [-.48,-.25,-.02,.21,.44]:b(x,-.045,-.1,.07,.01,.2,'paper')
 elif kind=='hanging_lamp':
  b(-.025,0,-.025,.05,.6,.05,'metal');b(-.62,-.12,-.31,1.24,.18,.62,'dark');b(-.5,-.18,-.24,1,.08,.48,'amber');b(-.57,.07,-.33,1.14,.04,.66,'steel')
 elif kind=='vent_fan':
  b(-.65,0,-.65,1.3,.1,1.3,'metal');b(-.53,-.02,-.53,1.06,.04,1.06,'glass')
  for i in range(6):
   z=-.48+i*.18;b(-.5,-.045,z,1,.04,.055,'edge')
 elif kind=='door_frame':
  b(-1.02,0,-.15,.22,3.25,.3,'steel');b(.80,0,-.15,.22,3.25,.3,'steel');b(-1.02,3.0,-.15,2.04,.3,.3,'metal');b(-1.06,.2,-.2,.06,2.5,.07,'cyan');b(1.00,.2,-.2,.06,2.5,.07,'cyan');b(-.42,3.06,-.21,.84,.13,.08,'green')
 elif kind=='door_panel':
  b(-.52,0,-.08,1.04,2.84,.16,'steel');b(-.47,.12,-.09,.94,2.58,.04,'metal');b(-.43,.65,-.105,.86,.8,.022,'glass');b(-.40,.8,-.12,.8,.04,.03,'cyan');b(-.08,1.7,-.14,.16,.20,.05,'amber');b(-.52,2.68,-.11,1.04,.1,.18,'dark')
 elif kind=='terminal':
  b(-.44,0,-.30,.88,.14,.58,'metal');b(-.32,.14,-.20,.64,1.06,.38,'dark');b(-.55,1.2,-.23,1.1,.88,.18,'metal');b(-.48,1.28,-.265,.96,.71,.06,'glass');b(-.41,1.85,-.277,.82,.045,.015,'cyan')
  for i in range(4):b(-.41,1.42+i*.10,-.279,.22+i*.13,.035,.016,'screen' if i%2 else 'green')
  b(-.43,1.06,-.49,.86,.1,.25,'steel')
  for i in range(5):
   for j in range(3):b(-.36+i*.16,1.17,-.44+j*.08,.1,.025,.045,'blue' if (i+j)%3==0 else 'edge')
 elif kind=='console_bank':
  b(-1.25,0,-.48,2.5,1.25,.96,'dark');b(-1.20,1.25,-.35,2.4,.20,.80,'metal')
  for i in range(3):
   x=-1.12+i*.8;b(x,1.45,-.28,.74,.86,.16,'steel');b(x+.06,1.53,-.30,.62,.67,.045,'glass')
   for j in range(4):b(x+.09,1.6+j*.13,-.305,.18+.1*(j%3),.035,.01,['cyan','green','amber'][j%3])
  for i in range(10):b(-1.10+i*.22,1.34,.12,.13,.03,.12,'edge')
 elif kind=='server_rack':
  b(-.56,0,-.46,1.12,3.35,.92,'dark');b(-.51,.12,-.50,1.02,3.05,.08,'metal')
  for i in range(13):
   y=.20+i*.225;b(-.46,y,-.51,.92,.17,.015,'steel');b(-.39,y+.045,-.533,.31,.025,.02,'glass')
   b(.29,y+.075,-.537,.045,.045,.016,'green' if i%3 else 'amber')
  for x in [-.58,.55]:b(x,0,-.51,.05,3.35,1.03,'edge')
 elif kind=='pipe_bundle':
  for i,(x,y,r) in enumerate([(-.55,.70,.18),(-.18,1.55,.13),(.28,1.05,.20),(.5,2.1,.12)]):
   b(x,y,-1,r*2,r*2,2.0,['steel','dark','metal','edge'][i]);b(x-.03,y-.03,-.56,r*2+.06,r*2+.06,.12,'amber')
  b(-.73,0,-.9,.13,2.65,.2,'metal');b(.70,0,.70,.13,2.65,.2,'metal')
 elif kind=='coolant_tank':
  b(-.60,0,-.55,1.2,.24,1.1,'metal');b(-.48,.24,-.48,.96,2.32,.96,'steel');b(-.62,2.52,-.62,1.24,.29,1.24,'metal')
  for i in range(7):b(-.49,.45+i*.30,-.51,.98,.065,.035,'dark')
  b(-.36,.97,-.54,.72,.66,.02,'glass');b(-.24,1.31,-.57,.48,.07,.04,'cyan')
 elif kind=='crate':
  b(-.48,0,-.48,.96,.85,.96,'steel');b(-.49,.1,-.49,.98,.08,.98,'dark');b(-.49,.66,-.49,.98,.08,.98,'dark')
  for x in [-.45,.37]:b(x,0,-.49,.08,.85,.98,'metal')
  b(-.24,.34,-.495,.48,.17,.015,'amber')
 elif kind=='chair':
  b(-.39,.5,-.37,.78,.16,.74,'rubber');b(-.38,.66,.22,.76,.95,.18,'metal');b(-.43,.85,.39,.86,.50,.08,'rubber')
  for x in [-.3,.25]:b(x,0,-.25,.075,.5,.075,'steel')
  b(-.44,.69,-.39,.10,.09,.65,'metal');b(.35,.69,-.39,.10,.09,.65,'metal')
 elif kind=='desk':
  b(-1.2,.78,-.43,2.4,.15,.86,'steel');b(-1.1,0,-.37,.16,.78,.7,'metal');b(.94,0,-.37,.16,.78,.7,'metal');b(-.90,.94,-.26,.92,.06,.6,'dark');b(.32,.96,-.12,.36,.03,.18,'paper')
 elif kind=='wall_monitor':
  b(-.74,.90,-.10,1.48,.96,.17,'metal');b(-.67,.98,-.135,1.34,.78,.04,'glass')
  for i in range(5):b(-.57,1.05+i*.13,-.148,.33+(i%3)*.2,.034,.012,['screen','cyan','amber','green','blue'][i])
  b(-.1,.87,-.14,.20,.03,.02,'red')
 elif kind=='warning_beacon':
  b(-.23,0,-.23,.46,.26,.46,'steel');b(-.14,.26,-.14,.28,.48,.28,'amber');b(-.23,.74,-.23,.46,.12,.46,'dark')
  for xx,zz in [(-.22,-.22),(.14,-.22),(-.22,.14),(.14,.14)]:b(xx,.28,zz,.07,.46,.07,'metal')
 elif kind=='relay':
  b(-.46,0,-.35,.92,1.2,.70,'dark');b(-.39,1.2,-.31,.78,.58,.62,'steel');b(-.29,1.4,-.39,.58,.23,.08,'amber')
  b(-.05,1.67,-.39,.10,.55,.10,'edge');b(-.12,2.14,-.41,.25,.10,.14,'red')
  for i in range(4):b(-.31+i*.18,1.28,-.37,.09,.07,.03,'cyan')
 elif kind=='record_station':
  b(-.5,0,-.4,1,.22,.8,'metal');b(-.34,.22,-.23,.68,.95,.48,'steel');b(-.61,1.20,-.28,1.22,.89,.24,'metal');b(-.53,1.29,-.30,1.06,.71,.035,'violet')
  for i in range(6):b(-.48,1.36+i*.09,-.325,.32+.11*(i%2),.025,.015,'paper' if i%2 else 'cyan')
 elif kind=='uplink':
  b(-.83,0,-.5,1.66,1.55,1.00,'metal');b(-.71,1.55,-.39,1.42,.74,.30,'steel');b(-.62,1.66,-.44,1.24,.52,.05,'green')
  for i in range(5):b(-.55,1.77+i*.07,-.46,.70-i*.06,.03,.02,'cyan')
  b(-.16,1.19,-.55,.32,.14,.13,'amber');b(-.92,0,-.42,.13,2.0,.15,'dark');b(.80,0,-.42,.13,2.0,.15,'dark')
 elif kind=='floor_grate':
  b(-.87,0,-.87,1.74,.055,1.74,'metal');b(-.76,.06,-.77,1.52,.03,1.54,'dark')
  for i in range(7):b(-.72+i*.22,.10,-.71,.09,.02,1.42,'steel')
 elif kind=='ceiling_duct':
  b(-.7,0,-1.0,1.4,.53,2.0,'steel');b(-.63,-.05,-1.03,1.26,.08,2.06,'dark')
  for i in range(5):b(-.72,0,-.95+i*.48,1.44,.55,.08,'metal')
 elif kind=='wall_sign':
  b(-.90,1.20,-.13,1.8,.56,.12,'metal');b(-.84,1.27,-.17,1.68,.42,.06,'dark')
  for i in range(4):b(-.72,1.34+i*.075,-.18,1.08-(i%2)*.24,.022,.017,'cyan')
  b(.48,1.38,-.19,.18,.14,.018,'amber')
 else:raise ValueError(kind)
 m.save('env_'+kind)

names=['ceiling_light','hanging_lamp','vent_fan','door_frame','door_panel','terminal','console_bank','server_rack','pipe_bundle','coolant_tank','crate','chair','desk','wall_monitor','warning_beacon','relay','record_station','uplink','floor_grate','ceiling_duct','wall_sign']
for n in names:a(n)
(root/'ENVIRONMENT_ASSETS.md').write_text('# Environmental models\n\n21 new authored low-poly `.obj` + `.mtl` assets.\n\n'+ '\n'.join('- `env_'+n+'`' for n in names)+'\n\nGenerated deterministically with `tools/make_environment_models.py`, with no third-party dependencies. Models use diffuse/emissive MTLs and reuse cached geometry in the renderer.\n')
# this source can be rerun directly to rebuild models; no copy is needed
