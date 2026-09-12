"""Generate OpenUU's dual-window logo assets from shared geometric paths.
Run from any directory with Python and Pillow installed.
"""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
NAVY = '#082b91'
BLUE = '#0877ff'
NODE = '#0755db'
# Quadratic paths also form the SVG source; no bitmap tracing.
BACK = [('M',82,40),('L',82,30),('Q',82,20,72,20),('L',24,20),('Q',14,20,14,30),('L',14,72),('Q',14,82,24,82),('L',36,82)]
FRONT = [('M',70,44),('L',106,44),('Q',116,44,116,54),('L',116,100),('Q',116,110,106,110),('L',62,110),('Q',52,110,52,100),('L',52,94)]

def path_text(commands):
    return ' '.join(c[0]+' '.join(map(str,c[1:])) for c in commands)

def mark_svg():
    return (f'<path d="{path_text(BACK)}" fill="none" stroke="{NAVY}" stroke-width="10" stroke-linecap="round"/>'
            f'<path d="{path_text(FRONT)}" fill="none" stroke="{BLUE}" stroke-width="10" stroke-linecap="round"/>'
            f'<rect x="42" y="58" width="23" height="25" rx="4" fill="{NODE}"/>')

def render(size, opaque=False):
    scale=8
    image=Image.new('RGBA',(128*scale,128*scale),'white' if opaque else (0,0,0,0))
    draw=ImageDraw.Draw(image)
    for commands,color in [(BACK,NAVY),(FRONT,BLUE)]:
        points=[]
        for command in commands:
            if command[0] in ('M','L'):
                current=command[1:]; points.append(current)
            else:
                x,y=current; cx,cy,ex,ey=command[1:]
                for i in range(1,33):
                    t=i/32
                    points.append(((1-t)**2*x+2*(1-t)*t*cx+t*t*ex,(1-t)**2*y+2*(1-t)*t*cy+t*t*ey))
                current=(ex,ey)
        draw.line([(x*scale,y*scale) for x,y in points],fill=color,width=10*scale,joint='curve')
        for x,y in points:
            draw.ellipse(((x-5)*scale,(y-5)*scale,(x+5)*scale,(y+5)*scale),fill=color)
    draw.rounded_rectangle((42*scale,58*scale,65*scale,83*scale),radius=4*scale,fill=NODE)
    return image.resize((size,size),Image.Resampling.LANCZOS)

def main():
    svg='<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">'+mark_svg()+'</svg>\n'
    for name in ['res/logo.svg','flutter/assets/icon.svg']:
        (ROOT/name).write_text(svg,encoding='utf-8')
    header='<svg xmlns="http://www.w3.org/2000/svg" width="440" height="128" viewBox="0 0 440 128">'+mark_svg()+'<text x="145" y="88" font-family="Segoe UI, sans-serif" font-size="64" font-weight="600" fill="#20242b">OpenUU</text></svg>\n'
    (ROOT/'res/logo-header.svg').write_text(header,encoding='utf-8')
    for name in ['res/icon.png','res/mac-icon.png','flutter/assets/icon.png','fastlane/metadata/android/en-US/images/icon.png']:
        render(1024).save(ROOT/name)
    for name in ['res/icon.ico','res/tray-icon.ico','flutter/windows/runner/resources/app_icon.ico']:
        render(256).save(ROOT/name,sizes=[(n,n) for n in [16,20,24,32,40,48,64,128,256]])
    for p in (ROOT/'flutter/ios/Runner/Assets.xcassets/AppIcon.appiconset').glob('*.png'):
        with Image.open(p) as old: size=old.width
        render(size,opaque=True).convert('RGB').save(p)

if __name__=='__main__': main()
