import json,subprocess,sys
f=sys.argv[1]; tc=sys.argv[2]
r=subprocess.run(['rustc','+'+tc,'--edition','2021','--emit=metadata','--error-format=json','-o','/dev/null',f],capture_output=True,text=True)
src=open(f,'rb').read(); n=0
for l in r.stderr.splitlines():
    try: d=json.loads(l)
    except: continue
    for c in [d]+d.get('children',[]):
        parts=[(s['byte_start'],s['byte_end'],s['suggested_replacement']) for s in c.get('spans',[]) if s.get('suggestion_applicability')=='MachineApplicable' and s.get('suggested_replacement') is not None]
        if not parts or d['level']!='warning': continue
        n+=1; fixed=bytearray(src)
        for a,b,t in sorted(parts,reverse=True): fixed[a:b]=t.encode()
        g=f.replace('.rs',f'_fix{n}.rs'); open(g,'wb').write(fixed)
        rr=subprocess.run(['rustc','+'+tc,'--edition','2021','--emit=metadata','-o','/dev/null',g],capture_output=True,text=True)
        errs=[x for x in rr.stderr.splitlines() if x.startswith('error')]
        print(f"  {d['code']['code'] if d.get('code') else '-'}: {[src[a:b].decode() for a,b,_ in parts]} -> {[t for _,_,t in parts]}  => {errs[0] if errs else 'compiles'}")
