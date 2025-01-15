#!/usr/bin/env python3

import socket
import struct
import sys

PROTO_BASE = 0

CLTOAN_CHART_DATA = (PROTO_BASE+506)
ANTOCL_CHART_DATA = (PROTO_BASE+507)

CS_CHARTS_UCPU = 0
CS_CHARTS_SCPU = 1
CS_CHARTS_MASTERIN = 2
CS_CHARTS_MASTEROUT = 3
CS_CHARTS_CSREPIN = 4
CS_CHARTS_CSREPOUT = 5
CS_CHARTS_CSSERVIN = 6
CS_CHARTS_CSSERVOUT = 7
CS_CHARTS_HDRBYTESR = 8
CS_CHARTS_HDRBYTESW = 9
CS_CHARTS_HDRLLOPR = 10
CS_CHARTS_HDRLLOPW = 11
CS_CHARTS_DATABYTESR = 12
CS_CHARTS_DATABYTESW = 13
CS_CHARTS_DATALLOPR = 14
CS_CHARTS_DATALLOPW = 15
CS_CHARTS_HLOPR = 16
CS_CHARTS_HLOPW = 17
CS_CHARTS_RTIME = 18
CS_CHARTS_WTIME = 19
CS_CHARTS_REPL = 20
CS_CHARTS_CREATE = 21
CS_CHARTS_DELETE = 22
CS_CHARTS_VERSION = 23
CS_CHARTS_DUPLICATE = 24
CS_CHARTS_TRUNCATE = 25
CS_CHARTS_DUPTRUNC = 26
CS_CHARTS_TEST = 27
CS_CHARTS_LOAD = 28
CS_CHARTS_MEMORY_RSS = 29
CS_CHARTS_MEMORY_VIRT = 30
CS_CHARTS_MOVELS = 31
CS_CHARTS_MOVEHS = 32
CS_CHARTS_CHANGE = 33
CS_CHARTS_SPLIT = 34
CS_CHARTS_USPACE = 35
CS_CHARTS_TSPACE = 36
CS_CHARTS_CHCOUNT = 37
CS_CHARTS_TDUSPACE = 38
CS_CHARTS_TDTSPACE = 39
CS_CHARTS_TDCHCOUNT = 40
CS_CHARTS_COPYCHUNKS = 41
CS_CHARTS_EC4CHUNKS = 42
CS_CHARTS_EC8CHUNKS = 43
CS_CHARTS_CNT = 44

MASTER_CHARTS_UCPU = 0
MASTER_CHARTS_SCPU = 1
MASTER_CHARTS_DELCHUNK = 2
MASTER_CHARTS_REPLCHUNK = 3
MASTER_CHARTS_STATFS = 4
MASTER_CHARTS_GETATTR = 5
MASTER_CHARTS_SETATTR = 6
MASTER_CHARTS_LOOKUP = 7
MASTER_CHARTS_MKDIR = 8
MASTER_CHARTS_RMDIR = 9
MASTER_CHARTS_SYMLINK = 10
MASTER_CHARTS_READLINK = 11
MASTER_CHARTS_MKNOD = 12
MASTER_CHARTS_UNLINK = 13
MASTER_CHARTS_RENAME = 14
MASTER_CHARTS_LINK = 15
MASTER_CHARTS_READDIR = 16
MASTER_CHARTS_OPEN = 17
MASTER_CHARTS_READCHUNK = 18
MASTER_CHARTS_WRITECHUNK = 19
MASTER_CHARTS_MEMORY_RSS = 20
MASTER_CHARTS_PACKETSRCVD = 21
MASTER_CHARTS_PACKETSSENT = 22
MASTER_CHARTS_BYTESRCVD = 23
MASTER_CHARTS_BYTESSENT = 24
MASTER_CHARTS_MEMORY_VIRT = 25
MASTER_CHARTS_USED_SPACE = 26
MASTER_CHARTS_TOTAL_SPACE = 27
MASTER_CHARTS_CREATECHUNK = 28
MASTER_CHARTS_CHANGECHUNK = 29
MASTER_CHARTS_DELETECHUNK_OK = 30
MASTER_CHARTS_DELETECHUNK_ERR = 31
MASTER_CHARTS_REPLICATECHUNK_OK = 32
MASTER_CHARTS_REPLICATECHUNK_ERR = 33
MASTER_CHARTS_CREATECHUNK_OK = 34
MASTER_CHARTS_CREATECHUNK_ERR = 35
MASTER_CHARTS_CHANGECHUNK_OK = 36
MASTER_CHARTS_CHANGECHUNK_ERR = 37
MASTER_CHARTS_SPLITCHUNK_OK = 38
MASTER_CHARTS_SPLITCHUNK_ERR = 39
MASTER_CHARTS_FILE_OBJECTS = 40
MASTER_CHARTS_META_OBJECTS = 41
MASTER_CHARTS_EC8_CHUNKS = 42
MASTER_CHARTS_EC4_CHUNKS = 43
MASTER_CHARTS_COPY_CHUNKS = 44
MASTER_CHARTS_REG_ENDANGERED = 45
MASTER_CHARTS_REG_UNDERGOAL = 46
MASTER_CHARTS_ALL_ENDANGERED = 47
MASTER_CHARTS_ALL_UNDERGOAL = 48
MASTER_CHARTS_BYTESREAD = 49
MASTER_CHARTS_BYTESWRITE = 50
MASTER_CHARTS_READ = 51
MASTER_CHARTS_WRITE = 52
MASTER_CHARTS_FSYNC = 53
MASTER_CHARTS_LOCK = 54
MASTER_CHARTS_SNAPSHOT = 55
MASTER_CHARTS_TRUNCATE = 56
MASTER_CHARTS_GETXATTR = 57
MASTER_CHARTS_SETXATTR = 58
MASTER_CHARTS_GETFACL = 59
MASTER_CHARTS_SETFACL = 60
MASTER_CHARTS_CREATE = 61
MASTER_CHARTS_META = 62
MASTER_CHARTS_CNT = 63

# single charts:
# percent , scale_base , mul , div , max
# default (0,0,1,1,0)

cs_single_charts = {}
cs_single_charts[MASTER_CHARTS_UCPU] = (1,-2,100,60,0)
cs_single_charts[MASTER_CHARTS_SCPU] = (1,-2,100,60,0)
cs_single_charts[CS_CHARTS_MASTERIN] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_MASTEROUT] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_CSREPIN] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_CSREPOUT] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_CSSERVIN] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_CSSERVOUT] = (0,-1,8000,60,0)
cs_single_charts[CS_CHARTS_HDRBYTESR] = (0,-1,1000,60,0)
cs_single_charts[CS_CHARTS_HDRBYTESW] = (0,-1,1000,60,0)
cs_single_charts[CS_CHARTS_DATABYTESR] = (0,-1,1000,60,0)
cs_single_charts[CS_CHARTS_DATABYTESW] = (0,-1,1000,60,0)
cs_single_charts[CS_CHARTS_RTIME] = (0,-2,1,60000,0)
cs_single_charts[CS_CHARTS_WTIME] = (0,-2,1,60000,0)
cs_single_charts[CS_CHARTS_LOAD] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_MEMORY_RSS] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_MEMORY_VIRT] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_USPACE] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_TSPACE] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_CHCOUNT] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_TDUSPACE] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_TDTSPACE] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_TDCHCOUNT] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_COPYCHUNKS] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_EC4CHUNKS] = (0,0,1,1,1)
cs_single_charts[CS_CHARTS_EC8CHUNKS] = (0,0,1,1,1)

master_single_charts = {}
master_single_charts[MASTER_CHARTS_UCPU] = (1,-2,100,60,0)
master_single_charts[MASTER_CHARTS_SCPU] = (1,-2,100,60,0)
master_single_charts[MASTER_CHARTS_PACKETSRCVD] = (0,-1,1000,60,0)
master_single_charts[MASTER_CHARTS_PACKETSSENT] = (0,-1,1000,60,0)
master_single_charts[MASTER_CHARTS_BYTESRCVD] = (0,-1,8000,60,0)
master_single_charts[MASTER_CHARTS_BYTESSENT] = (0,-1,8000,60,0)
master_single_charts[MASTER_CHARTS_BYTESREAD] = (0,-1,1000,60,0)
master_single_charts[MASTER_CHARTS_BYTESWRITE] = (0,-1,1000,60,0)
master_single_charts[MASTER_CHARTS_MEMORY_RSS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_MEMORY_VIRT] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_USED_SPACE] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_TOTAL_SPACE] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_FILE_OBJECTS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_META_OBJECTS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_EC8_CHUNKS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_EC4_CHUNKS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_COPY_CHUNKS] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_REG_ENDANGERED] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_REG_UNDERGOAL] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_ALL_ENDANGERED] = (0,0,1,1,1)
master_single_charts[MASTER_CHARTS_ALL_UNDERGOAL] = (0,0,1,1,1)

# multi charts:
# chart3 , chart2 , chart1 , joinmode , percent , scale_base , mul , div , max

cs_multi_charts = {}
cs_multi_charts[100] = (None,CS_CHARTS_UCPU,CS_CHARTS_SCPU,0,1,-2,100,60,0)
cs_multi_charts[101] = (None,CS_CHARTS_CSREPIN,CS_CHARTS_CSSERVIN,0,0,-1,8000,60,0)
cs_multi_charts[102] = (None,CS_CHARTS_CSREPOUT,CS_CHARTS_CSSERVOUT,0,0,-1,8000,60,0)
cs_multi_charts[103] = (None,CS_CHARTS_HDRBYTESR,CS_CHARTS_DATABYTESR,0,0,-1,1000,60,0)
cs_multi_charts[104] = (None,CS_CHARTS_HDRBYTESW,CS_CHARTS_DATABYTESW,0,0,-1,1000,60,0)
cs_multi_charts[105] = (None,CS_CHARTS_HDRLLOPR,CS_CHARTS_DATALLOPR,0,0,0,1,1,0)
cs_multi_charts[106] = (None,CS_CHARTS_HDRLLOPW,CS_CHARTS_DATALLOPW,0,0,0,1,1,0)
cs_multi_charts[107] = (None,CS_CHARTS_MEMORY_VIRT,CS_CHARTS_MEMORY_RSS,1,0,0,1,1,1)
cs_multi_charts[108] = (None,CS_CHARTS_MOVELS,CS_CHARTS_MOVEHS,0,0,0,1,1,0)
cs_multi_charts[109] = (None,CS_CHARTS_TSPACE,CS_CHARTS_USPACE,1,0,0,1,1,1)
cs_multi_charts[110] = (CS_CHARTS_COPYCHUNKS,CS_CHARTS_EC4CHUNKS,CS_CHARTS_EC8CHUNKS,0,0,0,1,1,1)

master_multi_charts = {}
master_multi_charts[100] = (None,MASTER_CHARTS_UCPU,MASTER_CHARTS_SCPU,0,1,-2,100,60,0)
master_multi_charts[101] = (None,MASTER_CHARTS_MEMORY_VIRT,MASTER_CHARTS_MEMORY_RSS,1,0,0,1,1,1)
master_multi_charts[102] = (None,MASTER_CHARTS_TOTAL_SPACE,MASTER_CHARTS_USED_SPACE,1,0,0,1,1,1)
master_multi_charts[103] = (None,MASTER_CHARTS_DELETECHUNK_OK,MASTER_CHARTS_DELETECHUNK_ERR,0,0,0,1,1,0)
master_multi_charts[104] = (None,MASTER_CHARTS_REPLICATECHUNK_OK,MASTER_CHARTS_REPLICATECHUNK_ERR,0,0,0,1,1,0)
master_multi_charts[105] = (None,MASTER_CHARTS_CREATECHUNK_OK,MASTER_CHARTS_CREATECHUNK_ERR,0,0,0,1,1,0)
master_multi_charts[106] = (None,MASTER_CHARTS_CHANGECHUNK_OK,MASTER_CHARTS_CHANGECHUNK_ERR,0,0,0,1,1,0)
master_multi_charts[107] = (None,MASTER_CHARTS_SPLITCHUNK_OK,MASTER_CHARTS_SPLITCHUNK_ERR,0,0,0,1,1,0)
master_multi_charts[108] = (None,MASTER_CHARTS_FILE_OBJECTS,MASTER_CHARTS_META_OBJECTS,0,0,0,1,1,1)
master_multi_charts[109] = (MASTER_CHARTS_COPY_CHUNKS,MASTER_CHARTS_EC4_CHUNKS,MASTER_CHARTS_EC8_CHUNKS,0,0,0,1,1,1)
master_multi_charts[110] = (None,MASTER_CHARTS_REG_UNDERGOAL,MASTER_CHARTS_REG_ENDANGERED,0,0,0,1,1,1)
master_multi_charts[111] = (None,MASTER_CHARTS_ALL_UNDERGOAL,MASTER_CHARTS_ALL_ENDANGERED,0,0,0,1,1,1)


nodata = (2**64)-1

if sys.version_info[0]<3 or ( sys.version_info[0]==3 and sys.version_info[1]<11):
	import cgi
	import cgitb

	cgitb.enable()

	fields = cgi.FieldStorage()
else:
	try:
		import urllib.parse as xurllib
	except ImportError:
		import urllib as xurllib
	import os

	class FieldStorage:
		def __init__(self):
			self.data = {}
			for k,v in xurllib.parse_qsl(os.environ["QUERY_STRING"]):
				if k in self.data:
					if type(self.data[k])==str:
						x = self.data[k]
						self.data[k] = [x]
					self.data[k].append(v)
				else:
					self.data[k] = v
		def __repr__(self):
			return repr(self.data)
		def __str__(self):
			return str(self.data)
		def __contains__(self,key):
			return key in self.data
		def __iter__(self):
			return iter(self.data.keys())
		def getvalue(self,key):
			if key in self.data:
				return self.data[key]
			return None

	fields = FieldStorage()

if "host" in fields:
	host = fields.getvalue("host")
else:
	host = ''
if "port" in fields:
	try:
		port = int(fields.getvalue("port"))
	except ValueError:
		port = 0
else:
	port = 0
if "id" in fields:
	try:
		chartid = int(fields.getvalue("id"))
	except ValueError:
		chartid = -1
else:
	chartid = -1
if "mode" in fields:
	try:
		mode = int(fields.getvalue("mode"))
	except ValueError:
		mode = 0
else:
	mode = 0

def mysend(socket,msg):
	totalsent = 0
	while totalsent < len(msg):
		sent = socket.send(msg[totalsent:])
		if sent == 0:
			raise RuntimeError("socket connection broken")
		totalsent = totalsent + sent

def myrecv(socket,leng):
	if sys.version<'3':
		msg = ''
	else:
		msg = bytes(0)
	while len(msg) < leng:
		chunk = socket.recv(leng-len(msg))
		if len(chunk) == 0:
			raise RuntimeError("socket connection broken")
		msg = msg + chunk
	return msg

def getsingledata(socket,chartid):
	mysend(socket,struct.pack(">LLLL",CLTOAN_CHART_DATA,8,chartid,4095))
	header = myrecv(socket,8)
	cmd,length = struct.unpack(">LL",header)
	data = myrecv(socket,length)
	if cmd==ANTOCL_CHART_DATA and length>=8:
		ts,e = struct.unpack(">LL",data[:8])
		if e*8+8==length:
			l = list(struct.unpack(">"+e*"Q",data[8:]))
			return ts,l
	return None,None

def getmultidata(socket,chartid):
	mysend(socket,struct.pack(">LLLLB",CLTOAN_CHART_DATA,9,chartid,4095,1))
	header = myrecv(socket,8)
	cmd,length = struct.unpack(">LL",header)
	data = myrecv(socket,length)
	if cmd==ANTOCL_CHART_DATA and length>=8:
		ranges,series,entries,perc,base = struct.unpack(">BBLBB",data[:8])
		if length==8+ranges*(13+series*entries*8):
			res = {}
			unpackstr = ">%uQ" % entries
			for r in range(ranges):
				rpos = 8 + r * (13+series*entries*8)
				rng,ts,mul,div = struct.unpack(">BLLL",data[rpos:rpos+13])
				rpos += 13
				if series>3:
					series=3
				l1 = None
				l2 = None
				l3 = None
				if series>=1:
					l1 = list(struct.unpack(unpackstr,data[rpos:rpos+entries*8]))
				if series>=2:
					l2 = list(struct.unpack(unpackstr,data[rpos+entries*8:rpos+2*entries*8]))
				if series>=3:
					l3 = list(struct.unpack(unpackstr,data[rpos+2*entries*8:rpos+3*entries*8]))
				res[rng] = (l1,l2,l3,ts,mul,div)
			return perc,base,res

def getchartparams(chnum):
	if mode==1:
		if chnum in master_multi_charts:
			return master_multi_charts[chnum]
		elif chnum in master_single_charts:
			return (None,None,chnum,0) + master_single_charts[chnum]
		else:
			return (None,None,chnum,0,0,0,1,1,0)
	elif mode==2:
		if chnum in cs_multi_charts:
			return cs_multi_charts[chnum]
		elif chnum in cs_single_charts:
			return (None,None,chnum,0) + cs_single_charts[chnum]
		else:
			return (None,None,chnum,0,0,0,1,1,0)
	else:
		return (None,None,chnum,0,0,0,1,1,0)

def dataarrtostr(arr,mul,div):
	res = []
	for v in arr:
		if v==nodata:
			res.append("null")
		else:
			res.append("%u" % v)
#			res.append("%.6lf" % ((v*mul)/div))
	return ("[%s]" % (",".join(res)))

def joinarr(arr1,arr2):
	res = []
	for v1,v2 in zip(arr1,arr2):
		if v1==nodata or v2==nodata:
			res.append((2**64)-1)
		elif v2>v1:
			res.append(v2-v1)
		else:
			res.append(0)
	return res

def senderr():
	print("Content-Type: text/json")
	print("")
	print("{")
	print("}")

if host=='' or port==0 or chartid<0:
	senderr()
else:
	try:
		if mode==0:
			s = socket.socket()
			s.settimeout(1)
			s.connect((host,port))
			perc,base,datadict = getmultidata(s,chartid)
			base -= 2
		else:
			s = socket.socket()
			s.settimeout(1)
			s.connect((host,port))
			chnum,chrange = divmod(chartid,10)
			datadict = {}
			if chrange==9:
				chrfrom = 0
				chrto = 4
			else:
				chrfrom = chrange
				chrto = chrange+1
			ch3,ch2,ch1,join,perc,base,mul,div,maxmode = getchartparams(chnum)
			for chrange in range(chrfrom,chrto):
				if maxmode:
					rdiv = 1
				else:
					rdiv = [1,6,30,1440][chrange]
				ch1data = None
				ch2data = None
				ch3data = None
				while True:
					ts1,ch1data = getsingledata(s,10*ch1+chrange)
					if ch2!=None:
						ts2,ch2data = getsingledata(s,10*ch2+chrange)
						if ch3!=None:
							ts3,ch3data = getsingledata(s,10*ch3+chrange)
						else:
							ts3 = ts2
					else:
						ts2 = ts1
						ts3 = ts1
					if ts1==ts2 and ts1==ts3:
						ts=ts1
						break
				if join:
					if ch2!=None:
						if ch3!=None:
							ch3data = joinarr(ch2data,ch3data)
						ch2data = joinarr(ch1data,ch2data)
				datadict[chrange] = (ch1data,ch2data,ch3data,ts,mul,div*rdiv)
		print('Content-Type: text/json')
		print('')
		print('{')
		for chrange in datadict:
			print('"%u":{' % chrange)
			ch1data,ch2data,ch3data,ts,mul,div = datadict[chrange]
			ch1str = dataarrtostr(ch1data,mul,div)
			if ch2data!=None:
				ch2str = dataarrtostr(ch2data,mul,div)
				if ch3data!=None:
					ch3str = dataarrtostr(ch3data,mul,div)
			print('		"dataarr1": %s,' % ch1str)
			if ch2data!=None:
				print('		"dataarr2": %s,' % ch2str)
				if ch3data!=None:
					print('		"dataarr3": %s,' % ch3str)
			print('		"timestamp": %u,' % ts)
			print('		"multiplier": %u,' % mul)
			print('		"divisor": %u' % div)
			print('	},')
		print('	"basescale": %d,' % base)
		print('	"percent": %s' % ("true" if perc else "false"))
		print('}')
		s.close()
	except Exception:
		senderr()
