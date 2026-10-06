import struct,sys,collections
def blocks(data):
    i=0
    while i+12<=len(data):
        t,l=struct.unpack_from('<II',data,i)
        if l<12: break
        yield t,data[i+8:i+l-4]
        i+=l
def packets(path):
    data=open(path,'rb').read(); links=[]
    for t,b in blocks(data):
        if t==1: links.append(struct.unpack_from('<H',b,0)[0])
        elif t==6:
            ifid,hi,lo,cap,orig=struct.unpack_from('<IIIII',b,0)
            yield links[ifid],((hi<<32)|lo),b[20:20+cap]
def decode(link,pkt):
    off=14 if link==1 else 0
    if link==1:
        et=struct.unpack_from('>H',pkt,12)[0]
        if et==0x8100: off=18; et=struct.unpack_from('>H',pkt,16)[0]
        if et!=0x0800: return None
    ip=pkt[off:]
    if len(ip)<20 or ip[0]>>4!=4: return None
    ihl=(ip[0]&15)*4; proto=ip[9]; src='.'.join(map(str,ip[12:16])); dst='.'.join(map(str,ip[16:20]))
    tot=struct.unpack_from('>H',ip,2)[0]; l4=ip[ihl:tot]
    if proto==6:
        sp,dp,seq,ack=struct.unpack_from('>HHII',l4,0); do=(l4[12]>>4)*4; fl=l4[13]
        return ('tcp',src,sp,dst,dp,seq,fl,l4[do:])
    if proto==17:
        sp,dp=struct.unpack_from('>HH',l4,0); return ('udp',src,sp,dst,dp,0,0,l4[8:])
    return (str(proto),src,0,dst,0,0,0,b'')
if __name__=='__main__':
    for f in sys.argv[1:]:
        print('==',f); c=collections.Counter(); b=collections.Counter(); n=0
        for link,ts,pkt in packets(f):
            n+=1; d=decode(link,pkt)
            if not d: continue
            k=(d[0],d[1],d[2],d[3],d[4]); c[k]+=1; b[k]+=len(d[7])
        print('packets',n)
        for k,v in c.most_common(15): print(' ',k,v,'pkts',b[k],'payload bytes')
