"""Optional visual inspection of detailed enemy OBJs. Requires Pillow (dev tool only).

The game itself uses only embedded OBJ/MTL data through mata-3d.
"""
from pathlib import Path
from PIL import Image
# Existing project previewer executes its original preview set upon import.
from runpy import run_path
preview=run_path(str(Path(__file__).parent/'render_preview.py'))['preview']
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'previews'
ENEMIES=[
 ('runner','RUNNER','High-speed interception robot',(3.05,2.45,4.3),(0,1.1,0)),
 ('striker','STRIKER','Armoured close-range assault robot',(3.55,2.75,4.8),(0,1.1,0)),
 ('tank','TANK','Heavy shielded siege platform',(4.3,3.2,5.2),(0,1.3,0)),
 ('marksman','MARKSMAN','Long-range optical tracking unit',(3.3,2.8,4.5),(0,1.2,0)),
 ('gunner','GUNNER','Dual weapon-pod suppression unit',(3.7,2.9,4.8),(0,1.2,0)),
]
images=[]
for name,title,subtitle,eye,target in ENEMIES:
 img=preview('enemy_'+name,title,subtitle,eye,target,size=(480,520),zoom=0.98)
 images.append(img)
canvas=Image.new('RGB',(1440,1080),(7,14,25))
for i,img in enumerate(images):
    x=(i%3)*480
    y=(i//3)*540
    if i>=3:x+=240
    canvas.paste(img,(x,y))
path=DEST/'detailed-enemies-contact-sheet.png'
canvas.save(path,optimize=True)
print('Preview contact sheet:',path)
