# Import PROTO_BASE from the main module
try:
    # First try to get it from builtins if it's been set there
    from builtins import masterhost
except ImportError:
    # If not in builtins, try to import from the parent package
    try:
        import sys
        sys.path.insert(0, '..')
        from mfscli_split import masterhost
    except ImportError:
        print("Could not import builtins masterhost")
        sys.exit(1)

# common auxiliary functions

def getmasteraddresses():
	m = []
	for mhost in masterhost.replace(';',' ').replace(',',' ').split():
		try:
			for i in socket.getaddrinfo(mhost,masterport,socket.AF_INET,socket.SOCK_STREAM,socket.SOL_TCP):
				if i[0]==socket.AF_INET and i[1]==socket.SOCK_STREAM and i[2]==socket.SOL_TCP:
					m.append(i[4])
		except Exception:
			pass
	return m

def disablesmask_to_string_list(disables_mask):
	cmds = ["chown","chmod","symlink","mkfifo","mkdev","mksock","mkdir","unlink","rmdir","rename","move","link","create","readdir","read","write","truncate","setlength","appendchunks","snapshot","settrash","setsclass","seteattr","setxattr","setfacl"]
	l = []
	m = 1
	for cmd in cmds:
		if disables_mask & m:
			l.append(cmd)
		m <<= 1
	return l

def disablesmask_to_string(disables_mask):
	return ",".join(disablesmask_to_string_list(disables_mask))

def state_name(stateid):
	if stateid==STATE_DUMMY:
		return "DUMMY"
	elif stateid==STATE_USURPER:
		return "USURPER"
	elif stateid==STATE_FOLLOWER:
		return "FOLLOWER"
	elif stateid==STATE_ELECT:
		return "ELECT"
	elif stateid==STATE_DEPUTY:
		return "DEPUTY"
	elif stateid==STATE_LEADER:
		return "LEADER"
	else:
		return "???"

def state_color(stateid,sync):
	if stateid==STATE_DUMMY:
		return 8
	elif stateid==STATE_FOLLOWER or stateid==STATE_USURPER:
		if sync:
			return 5
		else:
			return 6
	elif stateid==STATE_ELECT:
		return 3
	elif stateid==STATE_DEPUTY:
		return 2
	elif stateid==STATE_LEADER:
		return 4
	else:
		return 1

def gracetime2txt(gracetime):
	load_state_has_link = 0
	if gracetime>=0xC0000000:
		load_state = "Overloaded"
		load_state_msg = "Overloaded"
		load_state_info = "server queue is heavy loaded (overloaded)"
	elif gracetime>=0x80000000:
		load_state = "Rebalance"
		load_state_msg = "Rebalancing"
		load_state_info = "internal rebalance in progress"
	elif gracetime>=0x40000000:
		load_state = "Fast rebalance"
		load_state_msg = "Fast rebalancing"
		load_state_info = "high speed rebalance in progress"
	elif gracetime>0:
		load_state = "G(%u)" % gracetime
		load_state_msg = "%u secs graceful" % gracetime
		load_state_info = "server in graceful period - back to normal after %u seconds" % gracetime
		load_state_has_link = 1
	else:
		load_state = "Normal"
		load_state_msg = "Normal load"
		load_state_info = "server is responsive, working with low load"
	return (load_state, load_state_msg, load_state_info, load_state_has_link)

def decimal_number(number,sep=' '):
	parts = []
	while number>=1000:
		number,rest = divmod(number,1000)
		parts.append("%03u" % rest)
	parts.append(str(number))
	parts.reverse()
	return sep.join(parts)

def decimal_number_html(number):
	return decimal_number(number,"&#8239;")

def humanize_number(number,sep='',suff='B'):
	number*=100
	scale=0
	while number>=99950:
		number = number//1024
		scale+=1
	if number<995 and scale>0:
		b = (number+5)//10
		nstr = "%u.%u" % divmod(b,10)
	else:
		b = (number+50)//100
		nstr = "%u" % b
	if scale>0:
		return "%s%s%si%s" % (nstr,sep,"-KMGTPEZY"[scale],suff)
	else:
		return "%s%s%s" % (nstr,sep,suff)

def timeduration_to_shortstr(timeduration, sep=''):
	for l,s in ((604800,'w'),(86400,'d'),(3600,'h'),(60,'m'),(0,'s')):
		if timeduration>=l:
			if l>0:
				n = float(timeduration)/float(l)
			else:
				n = float(timeduration)
			rn = round(n,1)
			if n==round(n,0):
				return "%.0f%s%s" % (n,sep,s)
			else:
				return "%s%.1f%s%s" % (("~"+sep if n!=rn else ""),rn,sep,s)
	return "???"

def timeduration_to_fullstr(timeduration):
	if timeduration>=86400:
		days,dayseconds = divmod(timeduration,86400)
		daysstr = "%u day%s, " % (days,("s" if days!=1 else ""))
	else:
		dayseconds = timeduration
		daysstr = ""
	hours,hourseconds = divmod(dayseconds,3600)
	minutes,seconds = divmod(hourseconds,60)
	if seconds==round(seconds,0):
		return "%u second%s (%s%u:%02u:%02u)" % (timeduration,("" if timeduration==1 else "s"),daysstr,hours,minutes,seconds)
	else:
		seconds,fracsec = divmod(seconds,1)
		return "%.3f seconds (%s%u:%02u:%02u.%03u)" % (timeduration,daysstr,hours,minutes,seconds,round(1000*fracsec,0))

def hours_to_str(hours):
	days,hoursinday = divmod(hours,24)
	if days>0:
		if hoursinday>0:
			return "%ud %uh" % (days,hoursinday)
		else:
			return "%ud" % days
	else:
		return "%uh" % hours

def time_to_str(tm):
	return time.strftime("%Y-%m-%d %H:%M:%S",time.localtime(tm))

def label_id_to_char(id):
	return chr(ord('A')+id)

def labelmask_to_str(labelmask):
	str = ""
	m = 1
	for i in xrange(26):
		if labelmask & m:
			str += label_id_to_char(i)
		m <<= 1
	return str

def labelmasks_to_str(labelmasks):
	if labelmasks[0]==0:
		return "*"
	r = []
	for labelmask in labelmasks:
		if labelmask==0:
			break
		r.append(labelmask_to_str(labelmask))
	return "+".join(r)

def labelexpr_to_str(labelexpr):
	stack = []
	if labelexpr[0]==0:
		return "*"
	for i in labelexpr:
		if i==255:
			stack.append((0,'*'))
		elif i>=192 and i<255:
			n = (i-192)
			stack.append((0,chr(ord('A')+n)))
		elif i>=128 and i<192:
			n = (i-128)+2
			if n>len(stack):
				return 'EXPR ERROR'
			m = []
			for _ in xrange(n):
				l,s = stack.pop()
				if l>1:
					m.append("(%s)" % s)
				else:
					m.append(s)
			m.reverse()
			stack.append((1,"&".join(m)))
		elif i>=64 and i<128:
			n = (i-64)+2
			if n>len(stack):
				return 'EXPR ERROR'
			m = []
			for _ in xrange(n):
				l,s = stack.pop()
				if l>2:
					m.append("(%s)" % s)
				else:
					m.append(s)
			m.reverse()
			stack.append((2,"|".join(m)))
		elif i==1:
			if len(stack)==0:
				return 'EXPR ERROR'
			l,s = stack.pop()
			if l>0:
				stack.append((0,"~(%s)" % s))
			else:
				stack.append((0,"~%s" % s))
		elif i==0:
			break
		else:
			return 'EXPR ERROR'
	if len(stack)!=1:
		return 'EXPR ERROR'
	l,s = stack.pop()
	return s

def labellist_fold(labellist):
	ll = []
	prev_data = None
	count = 0
	for data in labellist:
		if (data != prev_data):
			if count>0:
				if count>1:
					ll.append(('%u%s' % (count,prev_data[0]),prev_data[1]))
				else:
					ll.append(prev_data)
			prev_data = data
			count = 1
		else:
			count = count+1
	if count>0:
		if count>1:
			ll.append(('%u%s' % (count,prev_data[0]),prev_data[1]))
		else:
			ll.append(prev_data)
	return ll

def uniqmask_to_str(uniqmask):
	if uniqmask==0:
		return "-"
	if uniqmask & UNIQ_MASK_IP:
		return "[IP]"
	if uniqmask & UNIQ_MASK_RACK:
		return "[RACK]"
	rstr = ""
	inrange = 0
	for i in xrange(26):
		if uniqmask & (1<<i):
			if inrange==0:
				rstr += chr(ord('A')+i)
				if i<24 and ((uniqmask>>i)&7)==7:
					inrange = 1
		else:
			if inrange==1:
				rstr += '-'
				rstr += chr(ord('A')+(i-1))
				inrange = 0
	if inrange:
		rstr += '-'
		rstr += chr(ord('A')+25)
	return rstr

def eattr_to_str(seteattr,clreattr):
	eattrs = ["noowner","noattrcache","noentrycache","nodatacache","snapshot","undeletable","appendonly","immutable"]
	outlist = []
	mask = 1
	for eattrname in eattrs:
		if seteattr&mask:
			outlist.append("+%s" % eattrname)
		if clreattr&mask:
			outlist.append("-%s" % eattrname)
		mask = mask << 1
	if len(outlist)>0:
		return ",".join(outlist)
	else:
		return "-"

def get_string_from_packet(data,pos,err):
	rstr = ""
	shift = 0
	if 1+pos > len(data):
		err = 1;
	if err==0:
		shift = data[pos]
		if shift+1+pos > len(data):
			err = 1
		else:
			rstr = data[pos+1:pos+1+shift]
		rstr = rstr.decode('utf-8','replace')
	return (rstr,pos+shift+1,err)

def get_longstring_from_packet(data,pos,err):
	rstr = ""
	shift = 0
	if 2+pos > len(data):
		err = 1;
	if err==0:
		shift = data[pos]*256+data[pos+1]
		if shift+2+pos > len(data):
			err = 1
		else:
			rstr = data[pos+2:pos+2+shift]
		rstr = rstr.decode('utf-8','replace')
	return (rstr,pos+shift+2,err)

def print_error(msg):
	if cgimode:
		out = []
		out.append("""<div class="tab_title ERROR">Oops!</div>""")
		out.append("""<table class="FR MESSAGE" cellspacing="0">""")
		out.append("""	<tr><td align="left"><span class="ERROR">An error has occurred:</span> %s</td></tr>""" % msg)
		out.append("""</table>""")
		print("\n".join(out))
	elif jsonmode:
		json_er_dict = {}
		json_er_errors = []
		json_er_errors.append({"message":msg})
		json_er_dict["errors"] = json_er_errors
		jcollect["errors"].append(json_er_dict)
	elif ttymode:
		tab = Table("Error",1)
		tab.header("message")
		tab.defattr("l")
		tab.append(msg)
		print("%s%s%s" % (colorcode[1],tab,ttyreset))
	else:
		print("""Oops, an error has occurred:""")
		print(msg)

def print_exception():
	exc_type, exc_value, exc_traceback = sys.exc_info()
	try:
		if cgimode:
			print("""<div class="tab_title ERROR">Oops!</div>""")
			print("""<table class="FR MESSAGE">""")
			print("""<tr><td align="left"><span class="ERROR">An error has occurred. Check your MooseFS configuration and network connections. </span><br/>If you decide to seek support because of this error, please include the following traceback:""")
			print("""<pre>""")
			print(traceback.format_exc().strip())
			print("""</pre></td></tr>""")
			print("""</table>""")
		elif jsonmode:
			json_er_dict = {}
			json_er_traceback = []
			for d in traceback.extract_tb(exc_traceback):
				json_er_traceback.append({"file":d[0] , "line":d[1] , "in":d[2] , "text":repr(d[3])})
			json_er_dict["traceback"] = json_er_traceback
			json_er_errors = []
			for d in traceback.format_exception_only(exc_type, exc_value):
				json_er_errors.append(repr(d.strip()))
			json_er_dict["errors"] = json_er_errors
			jcollect["errors"].append(json_er_dict)
		elif ttymode:
			tab = Table("Exception Traceback",4)
			tab.header("file","line","in","text")
			tab.defattr("l","r","l","l")
			for d in traceback.extract_tb(exc_traceback):
				tab.append(d[0],d[1],d[2],repr(d[3]))
			tab.append(("---","",4))
			tab.append(("Error","c",4))
			tab.append(("---","",4))
			for d in traceback.format_exception_only(exc_type, exc_value):
				tab.append((repr(d.strip()),"",4))
			print("%s%s%s" % (colorcode[1],tab,ttyreset))
		else:
			print("""---------------------------------------------------------------- error -----------------------------------------------------------------""")
			print(traceback.format_exc().strip())
			print("""----------------------------------------------------------------------------------------------------------------------------------------""")
	except Exception:
		print(traceback.format_exc().strip())

def version_convert(version):
	if version>=(4,0,0) and version<(4,11,0):
		return (version,0)
	elif version>=(2,0,0):
		return ((version[0],version[1],version[2]//2),version[2]&1)
	elif version>=(1,7,0):
		return (version,1)
	elif version>(0,0,0):
		return (version,0)
	else:
		return (version,-1)

def version_str_and_sort(version):
	version,pro = version_convert(version)
	strver = "%u.%u.%u" % version
	sortver = "%05u_%03u_%03u" % version
	if pro==1:
		strver += " PRO"
		sortver += "_2"
	elif pro==0:
		sortver += "_1"
	else:
		sortver += "_0"
	if strver == '0.0.0':
		strver = ''
	return (strver,sortver)

#Compares (stringified) versions ("4.56.3 PRO"), returns 1 if ver1>ver2, -1 if ver1<ver2, 0 if ver1==ver2, doesn't take into account "PRO" on not "PRO"
def cmp_ver(ver1, ver2):
	(ver1_1, ver1_2, ver1_3)=list(map(int, ver1.upper().replace("PRO","").split('.')))
	(ver2_1, ver2_2, ver2_3)=list(map(int, ver2.upper().replace("PRO","").split('.')))
	if (ver1_1>ver2_1):
		return 1
	elif (ver1_1<ver2_1):
		return -1
	if (ver1_2>ver2_2):
		return 1
	elif (ver1_2<ver2_2):
		return -1
	if (ver1_3>ver2_3):
		return 1
	elif (ver1_3<ver2_3):
		return -1
	return 0
