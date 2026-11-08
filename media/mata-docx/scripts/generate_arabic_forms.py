import unicodedata,regex
from pathlib import Path
sets={}
for cp in list(range(0xfb50,0xfe00))+list(range(0xfe70,0xff00)):
 ch=chr(cp);name=unicodedata.name(ch,'')
 for kind in ('ISOLATED','FINAL','INITIAL','MEDIAL'):
  if name.startswith('ARABIC LETTER ') and name.endswith(' '+kind+' FORM'):
   decomposed=unicodedata.normalize('NFKC',ch)
   if len(decomposed)==1: sets.setdefault(ord(decomposed),{})[kind]=cp
join_types=['Dual_Joining','Right_Joining','Left_Joining','Join_Causing','Transparent']
text=''.join(map(chr,range(0xd800)))+''.join(map(chr,range(0xe000,0x110000)))
out=Path(__file__).resolve().parent.parent/'src/arabic_data.rs'
with out.open('w') as f:
 f.write('// Generated Arabic presentation forms and Unicode Joining_Type classes.\n')
 f.write('pub static ARABIC_FORMS: &[(u32,[u32;4])] = &[\n')
 for base,forms in sorted(sets.items()):
  cols=[forms.get(x,0) for x in ('ISOLATED','FINAL','INITIAL','MEDIAL')]
  f.write(f'({base},[{",".join(map(str,cols))}]),\n')
 f.write('];\n')
 f.write('pub static JOIN_TYPES: &[(u32,u32,u8)] = &[\n')
 records=[]
 for i,kind in enumerate(join_types,1):
  for m in regex.finditer(r'\p{Joining_Type='+kind+r'}+',text):
   records.append((ord(text[m.start()]),ord(text[m.end()-1]),i))
 for a,b,k in sorted(records): f.write(f'({a},{b},{k}),\n')
 f.write('];\n')
print('Arabic form rows',len(sets),'joining intervals',len(records))
