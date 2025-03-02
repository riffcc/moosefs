import struct
from .constants import CLTOAN_CHART_DATA, ANTOCL_CHART_DATA

def charts_convert_data(datalist, mul, div, raw):
    """Convert raw chart data to appropriate format"""
    res = []
    nodata = (2**64)-1
    for v in datalist:
        if v == nodata:
            res.append(None)
        else:
            if raw:
                res.append(v)
            else:
                res.append((v*mul)/div)
    return res

def get_charts_multi_data(mfsconn, chartid, dataleng):
    """Get chart data from the MFS master server"""
    data, length = mfsconn.command(CLTOAN_CHART_DATA, ANTOCL_CHART_DATA, struct.pack(">LLB", chartid, dataleng, 1))
    if length >= 8:
        ranges, series, entries, perc, base = struct.unpack(">BBLBB", data[:8])
        if length == 8 + ranges * (13 + series * entries * 8):
            res = {}
            unpackstr = ">%uQ" % entries
            for r in range(ranges):
                rpos = 8 + r * (13 + series * entries * 8)
                rng, ts, mul, div = struct.unpack(">BLLL", data[rpos:rpos+13])
                rpos += 13
                if series > 3:
                    series = 3
                l1 = None
                l2 = None
                l3 = None
                if series >= 1:
                    l1 = list(struct.unpack(unpackstr, data[rpos:rpos+entries*8]))
                if series >= 2:
                    l2 = list(struct.unpack(unpackstr, data[rpos+entries*8:rpos+2*entries*8]))
                if series >= 3:
                    l3 = list(struct.unpack(unpackstr, data[rpos+2*entries*8:rpos+3*entries*8]))
                res[rng] = (l1, l2, l3, ts, mul, div)
            return perc, base, res
        else:
            return None, None, None
    else:
        return None, None, None 