import sys,collections,struct,hashlib
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from pcap_lite import packets,decode
def streams(path):
    flows=collections.OrderedDict()
    for link,ts,pkt in packets(path):
        d=decode(link,pkt)
        if not d or d[0]!='tcp' or not d[7]: continue
        _,src,sp,dst,dp,seq,fl,pl=d
        flows.setdefault((src,sp,dst,dp),{})[seq]=pl   # dedupe retransmits by seq
    out={}
    for k,segs in flows.items():
        buf=bytearray(); nxt=None
        for seq in sorted(segs):
            if nxt is None: nxt=seq
            if seq<nxt: 
                ov=nxt-seq; buf+=segs[seq][ov:]; nxt=seq+len(segs[seq]) if seq+len(segs[seq])>nxt else nxt
            else: buf+=segs[seq]; nxt=seq+len(segs[seq])
        out[k]=bytes(buf)
    return out
def lpd(buf):
    """Parse client->printer LPD stream: returns list of (kind,name,data)"""
    res=[];i=0
    if not buf or buf[0]!=2: return res
    j=buf.index(b'\n',i); res.append(('recvjob',buf[1:j].decode('latin1'),b'')); i=j+1
    while i<len(buf):
        c=buf[i]
        if c in (2,3):
            j=buf.index(b'\n',i); parts=buf[i+1:j].split(b' ',1); n=int(parts[0]); name=parts[1].decode('latin1')
            data=buf[j+1:j+1+n]; res.append(('ctrl' if c==2 else 'data',name,data)); i=j+1+n+1
        else: break
    return res
if __name__=='__main__':
    """usage: extract_lpr.py capture.pcapng [outdir]  -- list LPR (TCP 515) jobs; with outdir, save each data file"""
    import os
    path=sys.argv[1]; outdir=sys.argv[2] if len(sys.argv)>2 else None
    if outdir: os.makedirs(outdir,exist_ok=True)
    for k,b in streams(path).items():
        if k[3]!=515: continue
        print('stream',k,len(b),'bytes')
        for kind,name,data in lpd(b):
            print('  ',kind,repr(name),len(data),'bytes',hashlib.sha256(data).hexdigest()[:12])
            if kind=='data' and outdir:
                open(os.path.join(outdir,os.path.basename(path)+'_'+name+'.bin'),'wb').write(data)
            if kind=='ctrl': print('     ctrl:',data.decode('latin1').replace('\n',' | ')[:200])
