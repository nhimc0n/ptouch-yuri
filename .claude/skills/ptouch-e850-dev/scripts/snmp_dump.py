import sys
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from pcap_lite import packets,decode
def tlv(b,i):
    t=b[i]; l=b[i+1]; i+=2
    if l&0x80: n=l&0x7f; l=int.from_bytes(b[i:i+n],'big'); i+=n
    return t,b[i:i+l],i+l
def oid(b):
    f=b[0]; o=[f//40,f%40]; v=0
    for x in b[1:]:
        v=(v<<7)|(x&0x7f)
        if not x&0x80: o.append(v); v=0
    return '.'.join(map(str,o))
def val(t,v):
    if t==2: return int.from_bytes(v,'big',signed=True)
    if t==4:
        try: s=v.decode('ascii'); 
        except: s=None
        return s if s and s.isprintable() else v.hex(' ')
    if t==5: return 'null'
    if t==0x40: return '.'.join(map(str,v))
    if t in (0x41,0x42,0x43): return int.from_bytes(v,'big')
    if t in (0x80,0x81,0x82): return 'noSuch'
    return f't{t:02x}:{v.hex()}'
def parse(p):
    t,m,_=tlv(p,0); i=0
    _,ver,i=tlv(m,0); _,com,i=tlv(m,i); pt,pdu,_=tlv(m,i)
    _,rid,j=tlv(pdu,0); _,es,j=tlv(pdu,j); _,ei,j=tlv(pdu,j); _,vb,j=tlv(pdu,j)
    out=[];k=0
    while k<len(vb):
        _,x,k=tlv(vb,k); _,o,y=tlv(x,0); vt,vv,_=tlv(x,y); out.append((oid(o),val(vt,vv)))
    return pt,out
seen={}
for f in sys.argv[1:]:
    for link,ts,pkt in packets(f):
        d=decode(link,pkt)
        if d and d[0]=='udp' and 161 in (d[2],d[4]) and d[7]:
            try: pt,vbs=parse(d[7])
            except Exception as e: print('parse fail',e); continue
            kind={0xa0:'GET',0xa1:'GETNEXT',0xa2:'RESP',0xa3:'SET'}.get(pt,hex(pt))
            for o,v in vbs:
                key=(kind,o,str(v))
                if key in seen: seen[key]+=1; continue
                seen[key]=1; print(f.split('/')[-1],kind,o,'=',v)
