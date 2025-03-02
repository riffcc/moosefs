# Import necessary variables/modules that should be available from the main script
import os
import sys
import socket
import time
import codecs
import getopt
import builtins

# Try to get PROTO_BASE and other constants from builtins where they're published
try:
    from builtins import VERSION, cgimode
except ImportError:
    print("Error: Could not import from builtins")
    sys.exit(1)

#parse parameters and auxiliary functions
if cgimode:
	# in CGI mode set default output encoding to utf-8 (our html page encoding)
	if sys.version_info[1]<7:
		sys.stdout = codecs.getwriter("utf-8")(sys.stdout.detach())
	else:
		sys.stdout.reconfigure(encoding='utf-8')

	try:
		import urllib.parse as xurllib
	except ImportError:
		import urllib as xurllib

	class MFSFieldStorage:
		def __init__(self):
			self.data = {}
		def __repr__(self):
			return repr(self.data)
		def __str__(self):
			return str(self.data)
		def __contains__(self,key):
			return key in self.data
		def __iter__(self):
			return iter(self.data.keys())
		def initFromURL(self):
			for k,v in xurllib.parse_qsl(os.environ["QUERY_STRING"]):
				if k in self.data:
					if type(self.data[k])==str:
						x = self.data[k]
						self.data[k] = [x]
					self.data[k].append(v)
				else:
					self.data[k] = v
		def getvalue(self,key):
			if key in self.data:
				return self.data[key]
			return None
		def pop(self,key):
			return self.data.pop(key)
		def append(self, key, value):
			self.data[key]=value

	fields = MFSFieldStorage()
	fields.initFromURL()		

	ajax_request = fields.getvalue("ajax")=="container"
	if fields.getvalue("ajax")!=None:
		fields.pop("ajax") #prevent from including ajax URL param any further

	try:
		if "masterhost" in fields:
			masterhost = fields.getvalue("masterhost")
			if type(masterhost) is list:
				masterhost = ";".join(masterhost)
			masterhost = masterhost.replace('"','').replace('<','').replace('>','').replace("'",'').replace('&','').replace('%','')
		else:
			masterhost = 'mfsmaster'
			# Hack: Export PROTO_BASE to make it available to imported modules
			builtins.masterhost = masterhost
	except Exception:
		masterhost = 'mfsmaster'
		# Hack: Export PROTO_BASE to make it available to imported modules
		builtins.masterhost = masterhost
	try:
		masterport = int(fields.getvalue("masterport"))
	except Exception:
		masterport = 9421
	try:
		mastercontrolport = int(fields.getvalue("mastercontrolport"))
	except Exception:
		try:
			mastercontrolport = int(fields.getvalue("masterport"))-2
		except Exception:
			mastercontrolport = 9419
	try:
		if "mastername" in fields:
			mastername = str(fields.getvalue("mastername"))
		else:
			mastername = 'MooseFS'
	except Exception:
		mastername = 'MooseFS'

	def htmlentities(str):
		return str.replace('&','&amp;').replace('<','&lt;').replace('>','&gt;').replace("'",'&apos;').replace('"','&quot;')

	def urlescape(str):
		return xurllib.quote_plus(str)

	def resolve(strip):
		try:
			return (socket.gethostbyaddr(strip))[0]
		except Exception:
			return UNRESOLVED

	def createhtmllink(update):
		c = []
		for k in fields:
			if k not in update:
				f = fields.getvalue(k)
				if type(f) is list:
					for el in f:
						c.append("%s=%s" % (k,urlescape(el)))
				elif type(f) is str:
					c.append("%s=%s" % (k,urlescape(f)))
		for k,v in update.items():
			if v!="":
				c.append("%s=%s" % (k,urlescape(v)))
		return "mfs.cgi?%s" % ("&amp;".join(c))

	def createrawlink(update):
		c = []
		for k in fields:
			if k not in update:
				f = fields.getvalue(k)
				if type(f) is list:
					for el in f:
						c.append("%s=%s" % (k,urlescape(el)))
				elif type(f) is str:
					c.append("%s=%s" % (k,urlescape(f)))
		for k,v in update.items():
			if v!="":
				c.append("%s=%s" % (k,urlescape(v)))
		return ("mfs.cgi?%s" % ("&".join(c))).replace('"','').replace("'","")

	def createorderlink(prefix,columnid):
		ordername = "%sorder" % prefix
		revname = "%srev" % prefix
		try:
			orderval = int(fields.getvalue(ordername))
		except Exception:
			orderval = 0
		try:
			revval = int(fields.getvalue(revname))
		except Exception:
			revval = 0
		return createhtmllink({revname:"1"}) if orderval==columnid and revval==0 else createhtmllink({ordername:str(columnid),revname:"0"})

	def createinputs(ignorefields):
		for k in fields:
			if k not in ignorefields:
				f = fields.getvalue(k)
				if type(f) is list:
					for el in f:
						yield """<input type="hidden" name="%s" value="%s">""" % (k,htmlentities(el))
				elif type(f) is str:
					yield """<input type="hidden" name="%s" value="%s">""" % (k,htmlentities(f))
		return

else: # CLI mode
	import getopt

	masterhost = 'mfsmaster'
	masterport = 9421
	mastercontrolport = 9419
	mastername = 'MooseFS'
	frameset = -1
	plaintextseparator = "\t"
	forceplaintext = 0
	jsonmode = 0
	colormode = 0
	donotresolve = 0
	sectionset = []
	sectionsubset = []
	clicommands = []

# order and data parameters
	ICsclassid = -1
	ICmatrix = 0
	IMorder = 0
	IMrev = 0
	MForder = 0
	MFrev = 0
	CSorder = 0
	CSrev = 0
	MBorder = 0
	MBrev = 0
	HDdata = ""
	HDorder = 0
	HDrev = 0
	HDperiod = 0
	HDtime = 0
	HDaddrname = 1
	EXorder = 0
	EXrev = 0
	MSorder = 0
	MSrev = 0
	SCorder = 0
	SCrev = 0
	SCdata = 1
	PAorder = 0
	PArev = 0
	OForder = 0
	OFrev = 0
	OFsessionid = 0
	ALorder = 0
	ALrev = 0
	ALinode = 0
	MOorder = 0
	MOrev = 0
	MOdata = 0
	QUorder = 0
	QUrev = 0
	MCrange = 0
	MCcount = 25
	MCchdata = []
	CCrange = 0
	CCcount = 25
	CCchdata = []

	time_s = time.time()

# modes:
#  0 - percent (sum - cpu)
#  1 - ops/s (operations)
#  2 - humanized format in bytes (memory/disk space)
#  3,4 - not used in master
#  5 - in MB/s (data bytes read/written)
#  6 - raw data (number of chunks/files etc.)
#  7 - seconds
#  8 - percent (max - udiff)
	mcchartslist = [
			('ucpu',0,0,'User cpu usage'),
			('scpu',1,0,'System cpu usage'),
			('delete',2,1,'Number of chunk deletion attempts'),
			('replicate',3,1,'Number of chunk replication attempts'),
			('statfs',4,1,'Number of statfs operations'),
			('getattr',5,1,'Number of getattr operations'),
			('setattr',6,1,'Number of setattr operations'),
			('lookup',7,1,'Number of lookup operations'),
			('mkdir',8,1,'Number of mkdir operations'),
			('rmdir',9,1,'Number of rmdir operations'),
			('symlink',10,1,'Number of symlink operations'),
			('readlink',11,1,'Number of readlink operations'),
			('mknod',12,1,'Number of mknod operations'),
			('unlink',13,1,'Number of unlink operations'),
			('rename',14,1,'Number of rename operations'),
			('link',15,1,'Number of link operations'),
			('readdir',16,1,'Number of readdir operations'),
			('open',17,1,'Number of open operations'),
			('read_chunk',18,1,'Number of chunk_read operations'),
			('write_chunk',19,1,'Number of chunk_write operations'),
			('memoryrss',20,2,'Resident memory usage'),
			('prcvd',21,1,'Packets received by master'),
			('psent',22,1,'Packets sent by master'),
			('brcvd',23,5,'Bytes received by master'),
			('bsent',24,5,'Bytes sent by master'),
			('memoryvirt',25,2,'Virtual memory usage'),
			('usedspace',26,2,'RAW disk space usage'),
			('totalspace',27,2,'RAW disk space connected'),
			('create',28,1,'Number of chunk creation attempts'),
			('change',29,1,'Number of chunk internal operation attempts'),
			('delete_ok',30,1,'Number of successful chunk deletions'),
			('delete_err',31,1,'Number of unsuccessful chunk deletions'),
			('replicate_ok',32,1,'Number of successful chunk replications'),
			('replicate_err',33,1,'Number of unsuccessful chunk replications'),
			('create_ok',34,1,'Number of successful chunk creations'),
			('create_err',35,1,'Number of unsuccessful chunk creations'),
			('change_ok',36,1,'Number of successful chunk internal operations'),
			('change_err',37,1,'Number of unsuccessful chunk internal operations'),
			('split_ok',38,1,'Number of successful chunk split operations'),
			('split_err',39,1,'Number of unsuccessful chunk split operations'),
			('fileobjects',40,6,'Number of file object'),
			('metaobjects',41,6,'Number of non-file objects (directories,symlinks,etc.)'),
			('chunksec8',42,6,'Total number of chunks stored in EC8 format'),
			('chunksec4',43,6,'Total number of chunks stored in EC4 format'),
			('chunkscopy',44,6,'Total number of chunks stored in COPY format'),
			('chregdanger',45,6,'Number of endangered chunks (mark for removal excluded)'),
			('chregunder',46,6,'Number of undergoal chunks (mark for removal excluded)'),
			('challdanger',47,6,'Number of endangered chunks (mark for removal included)'),
			('challunder',48,6,'Number of undergoal chunks (mark for removal included)'),
			('bytesread',49,5,'Traffic from cluster (data + overhead), bytes per second'),
			('byteswrite',50,5,'Traffic to cluster (data + overhead), bytes per second'),
			('read',51,1,'Number of read operations'),
			('write',52,1,'Number of write operations'),
			('fsync',53,1,'Number of fsync operations'),
			('lock',54,1,'Number of lock operations'),
			('snapshot',55,1,'Number of snapshot operations'),
			('truncate',56,1,'Number of truncate operations'),
			('getxattr',57,1,'Number of getxattr operations'),
			('setxattr',58,1,'Number of setxattr operations'),
			('getfacl',59,1,'Number of getfacl operations'),
			('setfacl',60,1,'Number of setfacl operations'),
			('create',61,1,'Number of create operations'),
			('meta',62,1,'Number of extra metadata operations (sclass,trashretention,eattr etc.)'),
			('servers',64,6,'Number of all registered chunk servers (both connected and disconnected)'),
			('mdservers',65,6,'Number of disconnected chunk servers that are in maintenance mode'),
			('dservers',66,6,'Number of disconnected chunk servers that are not in maintenance mode'),
			('udiff',67,8,'Difference in space usage percent between the most and least used chunk server'),
			('mountbytrcvd',68,5,'Traffic from cluster (data only), bytes per second'),
			('mountbytsent',69,5,'Traffic to cluster (data only), bytes per second'),
			('cpu',100,0,'Cpu usage (total sys+user)')
	]
	mcchartsabr = {
			'delete':['del'],
			'replicate':['rep','repl'],
			'memoryrss':['memrss','rmem','mem'],
			'memoryvirt':['memvirt','vmem']
	}

# modes:
#  0 - percent (sum - cpu)
#  1 - ops/s (operations)
#  2 - humanized format in bytes (memory/disk space)
#  3 - threads (load)
#  4 - time
#  5 - in MB/s (data bytes read/written)
#  6 - raw data (number of chunks/files etc.)
#  7 - not used in CS
#  8 - percent (max - udiff)
	ccchartslist = [
			('ucpu',0,0,'User cpu usage'),
			('scpu',1,0,'System cpu usage'),
			('masterin',2,5,'Data received from master'),
			('masterout',3,5,'Data sent to master'),
			('csrepin',4,5,'Data received by replicator'),
			('csrepout',5,5,'Data sent by replicator'),
			('csservin',6,5,'Data received by csserv'),
			('csservout',7,5,'Data sent by csserv'),
			('hdrbytesr',8,5,'Bytes read (headers)'),
			('hdrbytesw',9,5,'Bytes written (headers)'),
			('hdrllopr',10,1,'Low level reads (headers)'),
			('hdrllopw',11,1,'Low level writes (headers)'),
			('databytesr',12,5,'Bytes read (data)'),
			('databytesw',13,5,'Bytes written (data)'),
			('datallopr',14,1,'Low level reads (data)'),
			('datallopw',15,1,'Low level writes (data)'),
			('hlopr',16,1,'High level reads'),
			('hlopw',17,1,'High level writes'),
			('rtime',18,4,'Read time'),
			('wtime',19,4,'Write time'),
			('repl',20,1,'Replicate chunk ops'),
			('create',21,1,'Create chunk ops'),
			('delete',22,1,'Delete chunk ops'),
			('version',23,1,'Set version ops'),
			('duplicate',24,1,'Duplicate ops'),
			('truncate',25,1,'Truncate ops'),
			('duptrunc',26,1,'Duptrunc (duplicate+truncate) ops'),
			('test',27,1,'Test chunk ops'),
			('load',28,3,'Server load'),
			('memoryrss',29,2,'Resident memory usage'),
			('memoryvirt',30,2,'Virtual memory usage'),
			('movels',31,1,'Low speed move ops'),
			('movehs',32,1,'High speed move ops'),
			('split',34,1,'Split ops'),
			('usedspace',35,2,'Used HDD space in bytes (mark for removal excluded)'),
			('totalspace',36,2,'Total HDD space in bytes (mark for removal excluded)'),
			('chunkcount',37,6,'Number of stored chunks (mark for removal excluded)'),
			('tdusedspace',38,2,'Used HDD space in bytes on disks marked for removal'),
			('tdtotalspace',39,2,'Total HDD space in bytes on disks marked for removal'),
			('tdchunkcount',40,6,'Number of chunks stored on disks marked for removal'),
			('copychunks',41,6,'Number of stored chunks (all disks)'),
			('ec4chunks',42,6,'Number of stored chunk parts in EC4 format (all disks)'),
			('ec8chunks',43,6,'Number of stored chunk parts in EC8 format (all disks)'),
			('hddok',44,6,'Number of valid folders (hard drives)'),
			('hddmfr',45,6,'Number of folders (hard drives) that are marked for removal'),
			('hdddmg',46,6,'Number of folders (hard drives) that are marked as damaged'),
			('udiff',47,8,'Difference in usage percent between the most and least used disk'),
			('cpu',100,0,'Cpu usage (total sys+user)')
	]
	ccchartsabr = {
			'memoryrss':['memrss','rmem','mem'],
			'memoryvirt':['memvirt','vmem']
	}

	jcollect = {
			"version": "4.57.5",
			"timestamp": time_s,
			"shifted_timestamp": shiftts(time_s),
			"timestamp_str": time.ctime(time_s),
			"dataset":{},
			"errors":[]
	}

	mccharts = {}
	cccharts = {}
	for name,no,mode,desc in mcchartslist:
		mccharts[name] = (no,mode,desc)
	for name,abrlist in mcchartsabr.items():
		for abr in abrlist:
			mccharts[abr] = mccharts[name]
	for name,no,mode,desc in ccchartslist:
		cccharts[name] = (no,mode,desc)
	for name,abrlist in ccchartsabr.items():
		for abr in abrlist:
			cccharts[abr] = cccharts[name]

	lastsval = ''
	lastorder = None
	lastrev = None
	lastid = None
	lastmode = None
	try:
		opts,args = getopt.getopt(sys.argv[1:],"hjvH:P:S:C:f:ps:no:rm:i:a:b:c:d:28")
	except Exception:
		opts = [('-h',None)]
	for opt,val in opts:
		if val==None:
			val=""
		if opt=='-h':
			print("usage:")
			print("\t%s [-hjpn28] [-H master_host] [-P master_port] [-f 0..3] -S(IN|IM|IG|MU|IC|IL|MF|CS|MB|HD|EX|MS|RS|SC|PA|OF|AL|MO|QU|MC|CC) [-s separator] [-o order_id [-r]] [-m mode_id] [i id] [-a master_data_count] [-b master_data_desc] [-c chunkserver_data_count] [-d chunkserver_data_desc]" % sys.argv[0])
			print("\t%s [-hjpn28] [-H master_host] [-P master_port] [-f 0..3] -C(RC/ip/port|TR/ip/port|BW/ip/port|M[01]/ip/port|RS/sessionid)" % sys.argv[0])
			print("\t%s -v" % sys.argv[0])
			print("\ncommon:\n")
			print("\t-h : print this message and exit")
			print("\t-v : print version number and exit")
			print("\t-j : print result in JSON format")
			print("\t-p : force plain text format on tty devices")
			print("\t-s separator : field separator to use in plain text format on tty devices (forces -p)")
			print("\t-2 : force 256-color terminal color codes")
			print("\t-8 : force 8-color terminal color codes")
			print("\t-H master_host : master address (default: mfsmaster)")
			print("\t-P master_port : master client port (default: 9421)")
			print("\t-n : do not resolve ip addresses (default when output device is not tty)")
			print("\t-f frame charset number : set frame charset to be displayed as table frames in ttymode")
			print("\t\t-f0 : use simple ascii frames '+','-','|' (default for non utf-8 encodings)")
			if (sys.stdout.encoding=='UTF-8' or sys.stdout.encoding=='utf-8'):
				print("\t\t-f1 : use utf-8 frames: \u250f\u2533\u2513\u2523\u254b\u252b\u2517\u253b\u251b\u2501\u2503\u2578\u2579\u257a\u257b")
				print("\t\t-f2 : use utf-8 frames: \u250c\u252c\u2510\u251c\u253c\u2524\u2514\u2534\u2518\u2500\u2502\u2574\u2575\u2576\u2577")
				print("\t\t-f3 : use utf-8 frames: \u2554\u2566\u2557\u2560\u256c\u2563\u255a\u2569\u255d\u2550\u2551 (default for utf-8 encodings)")
			else:
				print("\t\t-f1 : use utf-8 frames (thick single)")
				print("\t\t-f2 : use utf-8 frames (thin single)")
				print("\t\t-f3 : use utf-8 frames (double - default for utf-8 encodings)")
			print("\nmonitoring:\n")
			print("\t-S data set : defines data set to be displayed")
			print("\t\t-SIN : show full master info")
			print("\t\t-SIM : show only masters states")
			print("\t\t-SIG : show only general master info")
			print("\t\t-SMU : show only master memory usage")
			print("\t\t-SIC : show only chunks info (target/current redundancy level matrices)")
			print("\t\t-SIL : show only loop info (with messages)")
			print("\t\t-SMF : show only missing chunks/files")
			print("\t\t-SCS : show connected chunk servers")
			print("\t\t-SMB : show connected metadata backup servers")
			print("\t\t-SHD : show hdd data")
			print("\t\t-SEX : show exports")
			print("\t\t-SMS : show active mounts")
			print("\t\t-SRS : show resources (storage classes,open files,acquired locks)")
			print("\t\t-SSC : show storage classes")
			print("\t\t-SPA : show patterns override data")
			print("\t\t-SOF : show only open files")
			print("\t\t-SAL : show only acquired locks")
			print("\t\t-SMO : show operation counters")
			print("\t\t-SQU : show quota info")
			print("\t\t-SMC : show master charts data")
			print("\t\t-SCC : show chunkserver charts data")
			print("\t-o order_id : sort data by column specified by 'order id' (depends on data set)")
			print("\t-r : reverse order")
			print("\t-m mode_id : show data specified by 'mode id' (depends on data set)")
			print("\t-i id : storage class id for -SIN/SIC, sessionid for -SOF or inode for -SAL")
			print("\t-a master_data_count : how many master data entries should be shown")
			print("\t-b master_data_desc : define master data columns (prefix with '+' for raw data, prefix with 'ip:[port:]' for server choice, use 'all' for all available data)")
			print("\t-c chunkserver_data_count : how many chunkserver data entries should be shown")
			print("\t-d chunkserver_data_desc : define chunkserver data columns (prefix with '+' for raw data, prefix with 'ip:[port:]' for server choice, use 'all' for all available data)")
			print("\t\tmaster data columns:")
			for name,no,mode,desc in mcchartslist:
				if name in mcchartsabr:
					name = "%s,%s" % (name,",".join(mcchartsabr[name]))
				print("\t\t\t%s - %s" % (name,desc))
			print("\t\tchunkserver data columns:")
			for name,no,mode,desc in ccchartslist:
				if name in ccchartsabr:
					name = "%s,%s" % (name,",".join(ccchartsabr[name]))
				print("\t\t\t%s - %s" % (name,desc))
			print("\ncommands:\n")
			print("\t-C command : perform particular command")
			print("\t\t-CRC/ip/port : remove given chunkserver from list of active chunkservers")
			print("\t\t-CTR/ip/port : temporarily remove given chunkserver from list of active chunkservers (master elect only)")
			print("\t\t-CBW/ip/port : send given chunkserver back to work (from grace state)")
			print("\t\t-CM1/ip/port : switch given chunkserver to maintenance mode")
			print("\t\t-CM0/ip/port : switch given chunkserver to standard mode (from maintenance mode)")
			print("\t\t-CRS/sessionid : remove given session")
			sys.exit(0)
		elif opt=='-v':
			print("version: %s" % VERSION)
			sys.exit(0)
		elif opt=='-2':
			colormode = 2
		elif opt=='-8':
			colormode = 1
		elif opt=='-j':
			jsonmode = 1
		elif opt=='-p':
			forceplaintext = 1
		elif opt=='-s':
			plaintextseparator = val
			forceplaintext = 1
		elif opt=='-n':
			donotresolve = 1
		elif opt=='-f':
			try:
				frameset = int(val)
			except Exception:
				print("-f: wrong value")
				sys.exit(1)
		elif opt=='-H':
			masterhost = val
		elif opt=='-P':
			try:
				masterport = int(val)
			except Exception:
				print("-P: wrong value")
				sys.exit(1)
		elif opt=='-S':
			lastsval = val
			if 'IN' in val:
				sectionset.append("IN")
				sectionsubset.append("IM")
				sectionsubset.append("IG")
				sectionsubset.append("MU")
				sectionsubset.append("IC")
				sectionsubset.append("IL")
				sectionsubset.append("MF")
				if lastmode!=None:
					ICmatrix = lastmode
				if lastid!=None:
					ICsclassid = lastid
				if lastorder!=None:
					IMorder = lastorder
				if lastrev:
					IMrev = 1
			if 'IM' in val:
				sectionset.append("IN")
				sectionsubset.append("IM")
				if lastorder!=None:
					IMorder = lastorder
				if lastrev:
					IMrev = 1
			if 'IG' in val:
				sectionset.append("IN")
				sectionsubset.append("IG")
			if 'MU' in val:
				sectionset.append("IN")
				sectionsubset.append("MU")
			if 'IC' in val:
				sectionset.append("IN")
				sectionsubset.append("IC")
				if lastmode!=None:
					ICmatrix = lastmode
				if lastid!=None:
					ICsclassid = lastid
			if 'IL' in val:
				sectionset.append("IN")
				sectionsubset.append("IL")
			if 'MF' in val:
				sectionset.append("IN")
				sectionsubset.append("MF")
				if lastorder!=None:
					MForder = lastorder
				if lastrev:
					MFrev = 1
			if 'CS' in val:
				sectionset.append("CS")
				sectionsubset.append("CS")
				if lastorder!=None:
					CSorder = lastorder
				if lastrev:
					CSrev = 1
			if 'MB' in val:
				sectionset.append("CS")
				sectionsubset.append("MB")
				if lastorder!=None:
					MBorder = lastorder
				if lastrev:
					MBrev = 1
			if 'HD' in val:
				sectionset.append("HD")
				if lastorder!=None:
					HDorder = lastorder
				if lastrev:
					HDrev = 1
				if lastmode!=None:
					if lastmode>=0 and lastmode<6:
						HDperiod,HDtime = divmod(lastmode,2)
			if 'EX' in val:
				sectionset.append("EX")
				if lastorder!=None:
					EXorder = lastorder
				if lastrev:
					EXrev = 1
			if 'MS' in val:
				sectionset.append("MS")
				if lastorder!=None:
					MSorder = lastorder
				if lastrev:
					MSrev = 1
			if 'MO' in val:
				sectionset.append("MO")
				if lastorder!=None:
					MOorder = lastorder
				if lastrev:
					MOrev = 1
				if lastmode!=None:
					MOdata = lastmode
			if 'RS' in val:
				sectionset.append("RS")
				sectionsubset.append("SC")
				sectionsubset.append("PA")
				sectionsubset.append("OF")
				sectionsubset.append("AL")
			if 'SC' in val:
				sectionset.append("RS")
				sectionsubset.append("SC")
				if lastmode!=None:
					SCdata = lastmode
				if lastorder!=None:
					SCorder = lastorder
				if lastrev:
					SCrev = 1
			if 'PA' in val:
				sectionset.append("RS")
				sectionsubset.append("PA")
				if lastorder!=None:
					PAorder = lastorder
				if lastrev:
					PArev = 1
			if 'OF' in val:
				sectionset.append("RS")
				sectionsubset.append("OF")
				if lastorder!=None:
					OForder = lastorder
				if lastrev!=None:
					OFrev = 1
				if lastid!=None:
					OFsessionid = lastid
			if 'AL' in val:
				sectionset.append("RS")
				sectionsubset.append("AL")
				if lastorder!=None:
					ALorder = lastorder
				if lastrev!=None:
					ALrev = 1
				if lastid!=None:
					ALinode = lastid
			if 'QU' in val:
				sectionset.append("QU")
				if lastorder!=None:
					QUorder = lastorder
				if lastrev:
					QUrev = 1
			if 'MC' in val:
				sectionset.append("MC")
				if lastmode!=None:
					MCrange = lastmode
			if 'CC' in val:
				sectionset.append("CC")
				if lastmode!=None:
					CCrange = lastmode
			lastorder = None
			lastrev = 0
			lastmode = None
		elif opt=='-o':
			try:
				ival = int(val)
			except Exception:
				print("-o: wrong value")
				sys.exit(1)
			if 'IM' in lastsval:
				IMorder = ival
			if 'MF' in lastsval:
				MForder = ival
			if 'CS' in lastsval:
				CSorder = ival
			if 'MB' in lastsval:
				MBorder = ival
			if 'HD' in lastsval:
				HDorder = ival
			if 'EX' in lastsval:
				EXorder = ival
			if 'MS' in lastsval:
				MSorder = ival
			if 'MO' in lastsval:
				MOorder = ival
			if 'SC' in lastsval:
				SCorder = ival
			if 'PA' in lastsval:
				PAorder = ival
			if 'OF' in lastsval:
				OForder = ival
			if 'AL' in lastsval:
				ALorder = ival
			if 'QU' in lastsval:
				QUorder = ival
			if lastsval=='':
				lastorder = ival
		elif opt=='-r':
			if 'IM' in lastsval:
				IMrev = 1
			if 'MF' in lastsval:
				MFrev = 1
			if 'CS' in lastsval:
				CSrev = 1
			if 'MB' in lastsval:
				MBrev = 1
			if 'HD' in lastsval:
				HDrev = 1
			if 'EX' in lastsval:
				EXrev = 1
			if 'MS' in lastsval:
				MSrev = 1
			if 'MO' in lastsval:
				MOrev = 1
			if 'SC' in lastsval:
				SCrev = 1
			if 'PA' in lastsval:
				PArev = 1
			if 'OF' in lastsval:
				OFrev = 1
			if 'AL' in lastsval:
				ALrev = 1
			if 'QU' in lastsval:
				QUrev = 1
			if lastsval=='':
				lastrev = 1
		elif opt=='-m':
			try:
				ival = int(val)
			except Exception:
				print("-m: wrong value")
				sys.exit(1)
			if 'HD' in lastsval:
				if ival>=0 and ival<6:
					HDperiod,HDtime = divmod(ival,2)
				else:
					print("-m: wrong value")
					sys.exit(1)
			if 'MO' in lastsval:
				MOdata = ival
			if 'IN' in lastsval or 'IC' in lastsval:
				ICmatrix = ival
			if 'MC' in lastsval:
				MCrange = ival
			if 'CC' in lastsval:
				CCrange = ival
			if 'SC' in lastsval:
				SCdata = ival
			if lastsval=='':
				lastmode = ival
		elif opt=='-i':
			try:
				ival = int(val)
			except Exception:
				print("-i: wrong value")
				sys.exit(1)
			if 'OF' in lastsval:
				OFsessionid = ival
			if 'AL' in lastsval:
				ALinode = ival
			if 'IN' in lastsval or 'IC' in lastsval:
				ICsclassid = ival
			if lastsval=='':
				lastid = ival
		elif opt=='-a':
			try:
				MCcount = int(val)
			except Exception:
				print("-a: wrong value")
				sys.exit(1)
		elif opt=='-b':
			for x in val.split(','):
				x = x.strip()
				if ':' in x:
					xs = x.split(':')
					if len(xs)==2:
						chhost = xs[0]
						chport = 9421
						x = xs[1]
					elif len(xs)==3:
						chhost = xs[0]
						try:
							chport = int(xs[1])
						except Exception:
							print("master port: wrong value")
							sys.exit(1)
						x = xs[2]
					else:
						print("Wrong chart definition: %s" % x)
						sys.exit(0)
				else:
					chhost = None
					chport = None
				if x!='' and x[0]=='+':
					x = x[1:]
					rawmode = 1
				else:
					rawmode = 0
				if x in mccharts:
					MCchdata.append((chhost,chport,mccharts[x][0],mccharts[x][1],mccharts[x][2],x,rawmode))
				elif x=='all':
					for name,no,mode,desc in mcchartslist:
						if (no<100):
							MCchdata.append((chhost,chport,no,mode,desc,name,rawmode))
				else:
					print("Unknown master chart name: %s" % x)
					sys.exit(0)
		elif opt=='-c':
			try:
				CCcount = int(val)
			except Exception:
				print("-c: wrong value")
				sys.exit(1)
		elif opt=='-d':
			for x in val.split(','):
				x = x.strip()
				if ':' in x:
					xs = x.split(':')
					if len(xs)==2:
						chhost = xs[0]
						chport = 9422
						x = xs[1]
					elif len(xs)==3:
						chhost = xs[0]
						try:
							chport = int(xs[1])
						except Exception:
							print("chunkserver port: wrong value")
							sys.exit(1)
						x = xs[2]
					else:
						print("Unknown chart name: %s" % x)
						sys.exit(0)
				else:
					chhost = None
					chport = None
				if x!='' and x[0]=='+':
					x = x[1:]
					rawmode = 1
				else:
					rawmode = 0
#				if chhost==None or chport==None:
#					print("in chunkserver chart data server ip/host must be specified")
#					sys.exit(0)
				if x in cccharts:
					CCchdata.append((chhost,chport,cccharts[x][0],cccharts[x][1],cccharts[x][2],x,rawmode))
				elif x=='all':
					for name,no,mode,desc in ccchartslist:
						if (no<100):
							CCchdata.append((chhost,chport,no,mode,desc,name,rawmode))
				else:
					print("Unknown chunkserver chart name: %s" % x)
					sys.exit(0)
		elif opt=='-C':
			clicommands.append(val)

	if sectionset==[] and clicommands==[]:
		print("Specify data to be shown (option -S) or command (option -C). Use '-h' for help.")
		sys.exit(0)

	ttymode = 1 if forceplaintext==0 and os.isatty(1) and jsonmode==0 else 0
	if ttymode:
		try:
			import curses
			curses.setupterm()
			if curses.tigetnum("colors")>=256:
				colors256 = 1
			else:
				colors256 = 0
		except Exception:
			colors256 = 1 if 'TERM' in os.environ and '256' in os.environ['TERM'] else 0
		# colors: 0 - white,1 - red,2 - orange,3 - yellow,4 - green,5 - cyan,6 - blue,7 - violet,8 - gray
		CSI="\x1B["
		if colors256:
			ttyreset=CSI+"0m"
			colorcode=[CSI+"38;5;196m",CSI+"38;5;208m",CSI+"38;5;226m",CSI+"38;5;34m",CSI+"38;5;30m",CSI+"38;5;19m",CSI+"38;5;55m",CSI+"38;5;244m"]
		else:
			ttysetred=CSI+"31m"
			ttysetyellow=CSI+"33m"
			ttysetgreen=CSI+"32m"
			ttysetcyan=CSI+"36m"
			ttysetblue=CSI+"34m"
			ttysetmagenta=CSI+"35m"
			ttyreset=CSI+"0m"
			# no orange - use red, no gray - use white
			colorcode=[ttysetred,ttysetred,ttysetyellow,ttysetgreen,ttysetcyan,ttysetblue,ttysetmagenta,""]
	else:
		colorcode=["","","","","","","",""]

	if ttymode and (sys.stdout.encoding=='UTF-8' or sys.stdout.encoding=='utf-8'):
		if frameset>=0 and frameset<=3:
			tableframes=frameset
		else:
			tableframes=0
	else:
		tableframes=0

	# terminal encoding mambo jumbo (mainly replace unicode chars that can't be printed with '?' instead of throwing exception)
	term_encoding = sys.stdout.encoding
	if term_encoding==None:
		term_encoding = 'utf-8'
	if sys.version_info[1]<7:
		sys.stdout = codecs.getwriter(term_encoding)(sys.stdout.detach(),'replace')
		sys.stdout.encoding = term_encoding
	else:
		sys.stdout.reconfigure(errors='replace')

	# lines prepared for JSON output (force utf-8):
	#if sys.version_info[1]<7:
	#	sys.stdout = codecs.getwriter("utf-8")(sys.stdout.detach())
	#else:
	#	sys.stdout.reconfigure(encoding='utf-8')

	# frames:
	#  +-----+-----+-----+-----+
	#  |     |     |     |     |
	#  |     |     |     |  |  |
	#  |  +- | -+- | -+  |  +- |
	#  |  |  |  |  |  |  |  |  |
	#  |     |     |     |     |
	#  +-----+-----+-----+-----+
	#  |     |     |     |     |
	#  |  |  |  |  |  |  |  |  |
	#  | -+- | -+  |  +- | -+- |
	#  |  |  |  |  |     |     |
	#  |     |     |     |     |
	#  +-----+-----+-----+-----+
	#  |     |     |     |     |
	#  |  |  |     |  |  |     |
	#  | -+  | -+- |  +  | -+  |
	#  |     |     |  |  |     |
	#  |     |     |     |     |
	#  +-----+-----+-----+-----+
	#  |     |     |     |     |
	#  |  |  |     |     |     |
	#  |  +  |  +- |  +  |  +  |
	#  |     |     |  |  |     |
	#  |     |     |     |     |
	#  +-----+-----+-----+-----+
	#  

	class Table:
		Needseparator = 0
		def __init__(self,title,ccnt,defattr=""):
			if tableframes==1:
				self.frames = ['\u250f', '\u2533', '\u2513', '\u2523', '\u254b', '\u252b', '\u2517', '\u253b', '\u251b', '\u2501', '\u2503', '\u2578', '\u2579', '\u257a', '\u257b', ' ']
			elif tableframes==2:
				self.frames = ['\u250c', '\u252c', '\u2510', '\u251c', '\u253c', '\u2524', '\u2514', '\u2534', '\u2518', '\u2500', '\u2502', '\u2574', '\u2575', '\u2576', '\u2577', ' ']
			elif tableframes==3:
				self.frames = ['\u2554', '\u2566', '\u2557', '\u2560', '\u256c', '\u2563', '\u255a', '\u2569', '\u255d', '\u2550', '\u2551', ' ', '\u2551', ' ', '\u2551', ' ']
			else:
				self.frames = ['+','+','+','+','+','+','+','+','+','-','|',' ','|',' ','|',' ']
			self.title = title
			self.ccnt = ccnt
			self.head = []
			self.body = []
			self.defattrs = []
			self.cwidth = []
			for _ in range(ccnt):
				self.defattrs.append(defattr)
				self.cwidth.append(0)
		def combineattr(self,attr,defattr):
			attrcolor = ""
			for c in ("0","1","2","3","4","5","6","7","8"):
				if c in defattr:
					attrcolor = c
			for c in ("0","1","2","3","4","5","6","7","8"):
				if c in attr:
					attrcolor = c
			attrjust = ""
			for c in ("l","L","r","R","c","C"):
				if c in defattr:
					attrjust = c
			for c in ("l","L","r","R","c","C"):
				if c in attr:
					attrjust = c
			return attrcolor+attrjust
		def header(self,*rowdata):
			ccnt = 0
			for celldata in rowdata:
				if type(celldata) is tuple:
					if len(celldata)==3:
						ccnt+=celldata[2]
					else:
						if celldata[0]!=None:
							cstr = myunicode(celldata[0])
							if len(cstr) > self.cwidth[ccnt]:
								self.cwidth[ccnt] = len(cstr)
						ccnt+=1
				else:
					if celldata!=None:
						cstr = myunicode(celldata)
						if len(cstr) > self.cwidth[ccnt]:
							self.cwidth[ccnt] = len(cstr)
					ccnt+=1
			if ccnt != self.ccnt:
				raise IndexError
			self.head.append(rowdata)
		def defattr(self,*rowdata):
			if len(rowdata) != self.ccnt:
				raise IndexError
			self.defattrs = rowdata
		def append(self,*rowdata):
			ccnt = 0
			rdata = []
			for celldata in rowdata:
				if type(celldata) is tuple:
					if celldata[0]!=None:
						cstr = myunicode(celldata[0])
					else:
						cstr = ""
					if len(celldata)==3:
						rdata.append((cstr,self.combineattr(celldata[1],self.defattrs[ccnt]),celldata[2]))
						ccnt+=celldata[2]
					else:
						if len(cstr) > self.cwidth[ccnt]:
							self.cwidth[ccnt] = len(cstr)
						if len(celldata)==2:
							rdata.append((cstr,self.combineattr(celldata[1],self.defattrs[ccnt])))
						else:
							rdata.append((cstr,self.defattrs[ccnt]))
						ccnt+=1
				else:
					if celldata!=None:
						cstr = myunicode(celldata)
						if ccnt >= len(self.cwidth):
							raise IndexError("ccnt: %u, self.ccnt: %u, len(self.cwidth): %u" % (ccnt, self.ccnt, len(self.cwidth)))
						if len(cstr) > self.cwidth[ccnt]:
							self.cwidth[ccnt] = len(cstr)
						rdata.append((cstr,self.defattrs[ccnt]))
					else:
						rdata.append(celldata)
					ccnt+=1
			if ccnt != self.ccnt:
				raise IndexError("ccnt: %u, self.ccnt: %u" % (ccnt, self.ccnt))
			self.body.append(rdata)
		def attrdata(self,cstr,cattr,cwidth):
			retstr = ""
			if "1" in cattr:
				retstr += colorcode[0]
				needreset = 1
			elif "2" in cattr:
				retstr += colorcode[1]
				needreset = 1
			elif "3" in cattr:
				retstr += colorcode[2]
				needreset = 1
			elif "4" in cattr:
				retstr += colorcode[3]
				needreset = 1
			elif "5" in cattr:
				retstr += colorcode[4]
				needreset = 1
			elif "6" in cattr:
				retstr += colorcode[5]
				needreset = 1
			elif "7" in cattr:
				retstr += colorcode[6]
				needreset = 1
			elif "8" in cattr:
				retstr += colorcode[7]
				needreset = 1
			else:
				needreset = 0
			if cstr=="--":
				retstr += " "+"-"*cwidth+" "
			elif cstr=="---":
				retstr += "-"*(cwidth+2)
			elif "L" in cattr or "l" in cattr:
				retstr += " "+cstr.ljust(cwidth)+" "
			elif "R" in cattr or "r" in cattr:
				retstr += " "+cstr.rjust(cwidth)+" "
			else:
				retstr += " "+cstr.center(cwidth)+" "
			if needreset:
				retstr += ttyreset
			return retstr
		def lines(self):
			outstrtab = []
			if ttymode:
				tabdata = []
				# upper frame
				tabdata.append((("---","",self.ccnt),))
				# title
				tabdata.append(((self.title,"",self.ccnt),))
				# header
				if len(self.head)>0:
					tabdata.append((("---","",self.ccnt),))
					tabdata.extend(self.head)
				# head and data separator
				tabdata.append((("---","",self.ccnt),))
				# data
				if len(self.body)==0:
					tabdata.append((("no data","",self.ccnt),))
				else:
					tabdata.extend(self.body)
				# bottom frame
				tabdata.append((("---","",self.ccnt),))
				# check col-spaned headers and adjust column widths if necessary
				for rowdata in tabdata:
					ccnt = 0
					for celldata in rowdata:
						if type(celldata) is tuple and len(celldata)==3 and celldata[0]!=None:
							cstr = myunicode(celldata[0])
							clen = len(cstr)
							cwidth = sum(self.cwidth[ccnt:ccnt+celldata[2]])+3*(celldata[2]-1)
							if clen > cwidth:
								add = clen - cwidth
								adddm = divmod(add,celldata[2])
								cadd = adddm[0]
								if adddm[1]>0:
									cadd+=1
								for i in range(celldata[2]):
									self.cwidth[ccnt+i] += cadd
							ccnt += celldata[2]
						else:
							ccnt += 1
				separators = []
				# before tab - no separators
				seplist = []
				for i in range(self.ccnt+1):
					seplist.append(0)
				separators.append(seplist)
				for rowdata in tabdata:
					seplist = [1]
					for celldata in rowdata:
						if type(celldata) is tuple and len(celldata)==3:
							for i in range(celldata[2]-1):
								seplist.append(1 if celldata[0]=='---' else 0)
						seplist.append(1)
					separators.append(seplist)
				# after tab - no separators
				seplist = []
				for i in range(self.ccnt+1):
					seplist.append(0)
				separators.append(seplist)
				# add upper and lower separators:
				updownsep = [[a*2 + b for (a,b) in zip(x,y)] for (x,y) in zip(separators[2:],separators[:-2])]
				# create table
				for (rowdata,sepdata) in zip(tabdata,updownsep):
	#				print rowdata,sepdata
					ccnt = 0
					line = ""
					nsep = 0 #self.frames[10]
					for celldata in rowdata:
						cpos = ccnt
						cattr = ""
						if type(celldata) is tuple:
							if celldata[1]!=None:
								cattr = celldata[1]
							if len(celldata)==3:
								cwidth = sum(self.cwidth[ccnt:ccnt+celldata[2]])+3*(celldata[2]-1)
								ccnt+=celldata[2]
							else:
								cwidth = self.cwidth[ccnt]
								ccnt+=1
							cstr = celldata[0]
						else:
							cstr = celldata
							cwidth = self.cwidth[ccnt]
							ccnt+=1
						if cstr==None:
							cstr = ""
						cstr = myunicode(cstr)
						if cstr=="---":
							if nsep==0:
								line += self.frames[(13,6,0,3)[sepdata[cpos]]]
								#line += self.frames[(15,6,0,3)[sepdata[cpos]]]
							else:
								line += self.frames[(9,7,1,4)[sepdata[cpos]]]
							nsep = 1 #self.frames[4]
							for ci in range(cpos,ccnt-1):
								line += self.frames[9]*(self.cwidth[ci]+2)
								line += self.frames[(9,7,1,4)[sepdata[ci+1]]]
							line += self.frames[9]*(self.cwidth[ccnt-1]+2)
						else:
							if nsep==0:
								line += self.frames[(15,12,14,10)[sepdata[cpos]]]
								#line += self.frames[(15,10,10,10)[sepdata[cpos]]]
							else:
								line += self.frames[(11,8,2,5)[sepdata[cpos]]]
								#line += self.frames[(15,8,2,5)[sepdata[cpos]]]
							nsep = 0
							line += self.attrdata(cstr,cattr,cwidth)
					if nsep==0:
						line += self.frames[(15,12,14,10)[sepdata[ccnt]]]
						#line += self.frames[(15,10,10,10)[sepdata[ccnt]]]
					else:
						line += self.frames[(11,8,2,5)[sepdata[ccnt]]]
						#line += self.frames[(15,8,2,5)[sepdata[ccnt]]]
					outstrtab.append(line)
			else:
				for rowdata in self.body:
					row = []
					for celldata in rowdata:
						if type(celldata) is tuple:
							cstr = myunicode(celldata[0])
						elif celldata!=None:
							cstr = myunicode(celldata)
						else:
							cstr = ""
						row.append(cstr)
					outstrtab.append("%s:%s%s" % (self.title,plaintextseparator,plaintextseparator.join(row)))
			return outstrtab
		def __str__(self):
			if Table.Needseparator:
				sep = "\n"
			else:
				sep = ""
				Table.Needseparator = 1
			return sep+("\n".join(self.lines()))

	#x = Table("Test title",4)
	#x.header("column1","column2","column3","column4")
	#x.append("t1","t2","very long entry","test")
	#x.append(("r","r3"),("l","l2"),"also long entry","test")
	#print x
	#
	#x = Table("Very long table title",2)
	#x.defattr("l","r")
	#x.append("key","value")
	#x.append("other key",123)
	#y = []
	#y.append(("first","1"))
	#y.append(("second","4"))
	#x.append(*y)
	#print x
	#
	#x = Table("Table with complicated header",15,"r")
	#x.header(("","",4),("I/O stats last min","",8),("","",3))
	#x.header(("info","",4),("---","",8),("space","",3))
	#x.header(("","",4),("transfer","",2),("max time","",3),("# of ops","",3),("","",3))
	#x.header(("---","",15))
	#x.header("IP path","chunks","last error","status","read","write","read","write","fsync","read","write","fsync","used","total","used %")
	#x.append("192.168.1.102:9422:/mnt/hd4/",66908,"no errors","ok","19 MiB/s","27 MiB/s","263625 us","43116 us","262545 us",3837,3295,401,"1.0 TiB","1.3 TiB","76.41%")
	#x.append("192.168.1.102:9422:/mnt/hd5/",67469,"no errors","ok","25 MiB/s","29 MiB/s","340303 us","89168 us","223610 us",2487,2593,366,"1.0 TiB","1.3 TiB","75.93%")
	#x.append("192.168.1.111:9422:/mnt/hd5/",109345,("2012-10-12 07:27","2"),("damaged","1"),"-","-","-","-","-","-","-","-","1.2 TiB","1.3 TiB","87.18%")
	#x.append("192.168.1.211:9422:/mnt/hd5/",49128,"no errors",("marked for removal","4"),"-","-","-","-","-","-","-","-","501 GiB","1.3 TiB","36.46%")
	##x.append("192.168.1.111:9422:/mnt/hd5/",109345,("2012-10-12 07:27","2"),("damaged","1"),("","-",8),"1.2 TiB","1.3 TiB","87.18%")
	##x.append("192.168.1.211:9422:/mnt/hd5/",49128,"no errors",("marked for removal","4"),("","-",8),"501 GiB","1.3 TiB","36.46%")
	#x.append("192.168.1.229:9422:/mnt/hd10/","67969","no errors","ok","17 MiB/s","11 MiB/s","417292 us","76333 us","1171903 us","2299","2730","149","1.0 TiB","1.3 TiB","76.61%")
	#print x
	#
	#x = Table("Colors",1,"r")
	#x.append(("white","0"))
	#x.append(("red","1"))
	#x.append(("orange","2"))
	#x.append(("yellow","3"))
	#x.append(("green","4"))
	#x.append(("cyan","5"))
	#x.append(("blue","6"))
	#x.append(("magenta","7"))
	#x.append(("gray","8"))
	#print x
	#
	#x = Table("Adjustments",1)
	#x.append(("left","l"))
	#x.append(("right","r"))
	#x.append(("center","c"))
	#print x
	#
	#x = Table("Special entries",3)
	#x.defattr("l","r","r")
	#x.header("entry","effect","extra column")
	#x.append("-- ","--","")
	#x.append("--- ","---","")
	#x.append("('--','',2)",('--','',2))
	#x.append("('','',2)",('','',2))
	#x.append("('---','',2)",('---','',2))
	#x.append("('red','1')",('red','1'),'')
	#x.append("('orange','2')",('orange','2'),'')
	#x.append("('yellow','3')",('yellow','3'),'')
	#x.append("('green','4')",('green','4'),'')
	#x.append("('cyan','5')",('cyan','5'),'')
	#x.append("('blue','6')",('blue','6'),'')
	#x.append("('magenta','7')",('magenta','7'),'')
	#x.append("('gray','8')",('gray','8'),'')
	#x.append(('---','',3))
	#x.append("('left','l',2)",('left','l',2))
	#x.append("('right','r',2)",('right','r',2))
	#x.append("('center','c',2)",('center','c',2))
	#print x

	def resolve(strip):
		if donotresolve:
			return strip
		try:
			return (socket.gethostbyaddr(strip))[0]
		except Exception:
			return strip
