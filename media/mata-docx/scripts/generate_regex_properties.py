from pathlib import Path
import regex
text=''.join(map(chr,range(0xd800)))+''.join(map(chr,range(0xe000,0x110000)))
names=['Lu','Ll','Lt','Lm','Lo','Mn','Mc','Me','Nd','Nl','No','Pc','Pd','Ps','Pe','Pi','Pf','Po','Sm','Sc','Sk','So','Zs','Zl','Zp','Cc','Cf','Cs','Co']
values=[]
for i,n in enumerate(names,1):
 for m in regex.finditer(r'\p{gc='+n+r'}+',text):
  values.append((ord(text[m.start()]),ord(text[m.end()-1]),i))
values.sort()
output=Path(__file__).resolve().parent.parent/'src/ucd_regex.rs'
with output.open('w') as f:
 f.write('// Generated Unicode general category data; Rust compile is std-only.\n')
 f.write('pub static GENERAL_CATEGORY_NAMES:&[&str]=&["Cn",'+','.join('"'+n+'"' for n in names)+'];\n')
 f.write('pub static GENERAL_CATEGORY:&[(u32,u32,u8)]=&[\n')
 for a,b,i in values:f.write(f'({a},{b},{i}),\n')
 f.write('];\n')
print('General category intervals:',len(values))
