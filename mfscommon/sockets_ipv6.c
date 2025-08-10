/*
 * Copyright (C) 2025 Jakub Kruszona-Zawadzki, Saglabs SA
 * 
 * IPv6 Support Extensions
 * Copyright (C) 2025 Benjamin Arntzen <zorlin@gmail.com>
 * 
 * This file is part of MooseFS.
 * 
 * MooseFS is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 2 (only).
 * 
 * MooseFS is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with MooseFS; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA 02111-1301, USA
 * or visit http://www.gnu.org/licenses/gpl-2.0.html
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <inttypes.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <errno.h>
#include <poll.h>
#include <sys/ioctl.h>

#include "sockets.h"
#include "sockets_ipv6.h"
#include "clocks.h"

/* -------------- Helper Functions -------------- */

static inline int descnonblock(int sock) {
#ifdef O_NONBLOCK
    int flags = fcntl(sock, F_GETFL, 0);
    if (flags == -1) {
        return -1;
    }
    return fcntl(sock, F_SETFL, flags | O_NONBLOCK);
#else
    int yes = 1;
    return ioctl(sock, FIONBIO, &yes);
#endif
}

/* -------------- IPv6 String Conversion -------------- */

void univ6makestrip(char strip[STRIPSIZE_V6], const mfs_ip *ip) {
    if (ip->family == AF_INET) {
        struct in_addr addr;
        addr.s_addr = htonl(ip->addr.v4);
        inet_ntop(AF_INET, &addr, strip, STRIPSIZE_V6);
    } else if (ip->family == AF_INET6) {
        inet_ntop(AF_INET6, ip->addr.v6, strip, STRIPSIZE_V6);
    } else {
        strip[0] = '\0';
    }
}

void univ6makestripport(char stripport[STRIPPORTSIZE_V6], const mfs_ip *ip, uint16_t port) {
    char strip[STRIPSIZE_V6];
    univ6makestrip(strip, ip);
    
    if (ip->family == AF_INET6) {
        snprintf(stripport, STRIPPORTSIZE_V6, "[%s]:%u", strip, port);
    } else {
        snprintf(stripport, STRIPPORTSIZE_V6, "%s:%u", strip, port);
    }
}

char* univ6allocstrip(const mfs_ip *ip) {
    char strip[STRIPSIZE_V6];
    univ6makestrip(strip, ip);
    return strdup(strip);
}

char* univ6allocstripport(const mfs_ip *ip, uint16_t port) {
    char stripport[STRIPPORTSIZE_V6];
    univ6makestripport(stripport, ip, port);
    return strdup(stripport);
}

/* -------------- mfs_ip Utility Functions -------------- */

void ipv4_to_mfs_ip(mfs_ip *mip, uint32_t ipv4) {
    mip->family = AF_INET;
    mip->addr.v4 = ipv4;
}

uint32_t mfs_ip_to_ipv4(const mfs_ip *mip) {
    if (mip->family == AF_INET) {
        return mip->addr.v4;
    }
    return 0; // Can't convert IPv6 to IPv4
}

int mfs_ip_is_v4(const mfs_ip *mip) {
    return (mip->family == AF_INET);
}

int mfs_ip_is_v6(const mfs_ip *mip) {
    return (mip->family == AF_INET6);
}

int mfs_ip_equal(const mfs_ip *a, const mfs_ip *b) {
    if (a->family != b->family) {
        return 0;
    }
    
    if (a->family == AF_INET) {
        return (a->addr.v4 == b->addr.v4);
    } else if (a->family == AF_INET6) {
        return (memcmp(a->addr.v6, b->addr.v6, 16) == 0);
    }
    
    return 0;
}

/* -------------- TCP IPv6 Functions -------------- */

int tcp6socket(void) {
    int sock = socket(AF_INET6, SOCK_STREAM, 0);
    if (sock >= 0) {
        // Enable dual-stack (IPv4 and IPv6 on same socket)
        int zero = 0;
        setsockopt(sock, IPPROTO_IPV6, IPV6_V6ONLY, &zero, sizeof(zero));
    }
    return sock;
}

int udp6socket(void) {
    int sock = socket(AF_INET6, SOCK_DGRAM, 0);
    if (sock >= 0) {
        // Enable dual-stack (IPv4 and IPv6 on same socket)
        int zero = 0;
        setsockopt(sock, IPPROTO_IPV6, IPV6_V6ONLY, &zero, sizeof(zero));
    }
    return sock;
}

/* -------------- IP Format Conversion -------------- */

/* -------------- Socket Address Helpers -------------- */

static int mfs_sockaddr_fill(mfs_sockaddr *msa, const mfs_ip *ip, uint16_t port) {
    memset(msa, 0, sizeof(mfs_sockaddr));
    
    if (ip->family == AF_INET) {
        msa->family = AF_INET;
        msa->addr.v4.sin_family = AF_INET;
        msa->addr.v4.sin_port = htons(port);
        msa->addr.v4.sin_addr.s_addr = htonl(ip->addr.v4);
        msa->addrlen = sizeof(struct sockaddr_in);
#ifdef HAVE_SOCKADDR_SIN_LEN
        msa->addr.v4.sin_len = sizeof(struct sockaddr_in);
#endif
    } else if (ip->family == AF_INET6) {
        msa->family = AF_INET6;
        msa->addr.v6.sin6_family = AF_INET6;
        msa->addr.v6.sin6_port = htons(port);
        memcpy(&msa->addr.v6.sin6_addr, ip->addr.v6, 16);
        msa->addrlen = sizeof(struct sockaddr_in6);
#ifdef HAVE_SOCKADDR_SIN6_LEN
        msa->addr.v6.sin6_len = sizeof(struct sockaddr_in6);
#endif
    } else {
        return -1;
    }
    
    return 0;
}

static int mfs_sockaddr_resolve(mfs_sockaddr *msa, const char *hostname, const char *service, int socktype, int passive) {
    struct addrinfo hints, *res, *reshead;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;  // Allow both IPv4 and IPv6
    hints.ai_socktype = socktype;
    hints.ai_flags = AI_V4MAPPED | AI_ADDRCONFIG;
    
    if (passive) {
        hints.ai_flags |= AI_PASSIVE;
    }
    
    if (hostname && hostname[0] == '*') {
        hostname = NULL;
    }
    if (service && service[0] == '*') {
        service = NULL;
    }
    
    if (getaddrinfo(hostname, service, &hints, &reshead)) {
        return -1;
    }
    
    // Prefer IPv6 if available
    for (res = reshead; res; res = res->ai_next) {
        if (res->ai_family == AF_INET6) {
            msa->family = AF_INET6;
            msa->addr.v6 = *((struct sockaddr_in6*)(res->ai_addr));
            msa->addrlen = res->ai_addrlen;
            freeaddrinfo(reshead);
            return 0;
        }
    }
    
    // Fall back to IPv4
    for (res = reshead; res; res = res->ai_next) {
        if (res->ai_family == AF_INET) {
            msa->family = AF_INET;
            msa->addr.v4 = *((struct sockaddr_in*)(res->ai_addr));
            msa->addrlen = res->ai_addrlen;
            freeaddrinfo(reshead);
            return 0;
        }
    }
    
    freeaddrinfo(reshead);
    return -1;
}

/* ----------------- TCP IPv6 ----------------- */

int tcp6resolve(const char *hostname, const char *service, mfs_ip *ip, uint16_t *port, int passiveflag) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, passiveflag) < 0) {
        return -1;
    }
    
    if (ip != NULL) {
        if (msa.family == AF_INET) {
            ip->family = AF_INET;
            ip->addr.v4 = ntohl(msa.addr.v4.sin_addr.s_addr);
        } else if (msa.family == AF_INET6) {
            ip->family = AF_INET6;
            memcpy(ip->addr.v6, &msa.addr.v6.sin6_addr, 16);
        }
    }
    
    if (port != NULL) {
        if (msa.family == AF_INET) {
            *port = ntohs(msa.addr.v4.sin_port);
        } else if (msa.family == AF_INET6) {
            *port = ntohs(msa.addr.v6.sin6_port);
        }
    }
    
    return 0;
}

int tcp6numbind(int sock, const mfs_ip *ip, uint16_t port) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    return bind(sock, &msa.addr.generic, msa.addrlen);
}

int tcp6strbind(int sock, const char *hostname, const char *service) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, 1) < 0) {
        return -1;
    }
    return bind(sock, &msa.addr.generic, msa.addrlen);
}

int tcp6numconnect(int sock, const mfs_ip *ip, uint16_t port) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    int result = connect(sock, &msa.addr.generic, msa.addrlen);
    if (result >= 0) {
        return 0;  // Immediate connection success
    }
    // Handle non-blocking socket case - IPv6 FIX APPLIED
    if (errno == EINPROGRESS) {
        return 1;  // Connection in progress (non-blocking socket)
    }
    return -1;  // Actual connection error
}

int tcp6numtoconnect(int sock, const mfs_ip *ip, uint16_t port, uint32_t msecto) {
    mfs_sockaddr msa;
    struct pollfd pfd;
    double st;
    int res;
    
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    
    if (descnonblock(sock) < 0) {
        return -1;
    }
    
    res = connect(sock, &msa.addr.generic, msa.addrlen);
    if (res == 0) {
        return 0;
    }
    
    if (errno != EINPROGRESS) {
        return -1;
    }
    
    st = monotonic_seconds();
    for (;;) {
        pfd.fd = sock;
        pfd.events = POLLOUT;
        pfd.revents = 0;
        
        res = poll(&pfd, 1, msecto);
        if (res > 0) {
            return tcpgetstatus(sock);
        }
        if (res == 0) {
            errno = ETIMEDOUT;
            return -1;
        }
        if (errno != EINTR) {
            return -1;
        }
        
        if (monotonic_seconds() - st >= msecto * 0.001) {
            errno = ETIMEDOUT;
            return -1;
        }
    }
}

int tcp6strconnect(int sock, const char *hostname, const char *service) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, 0) < 0) {
        return -1;
    }
    return connect(sock, &msa.addr.generic, msa.addrlen);
}

int tcp6strtoconnect(int sock, const char *hostname, const char *service, uint32_t msecto) {
    mfs_sockaddr msa;
    struct pollfd pfd;
    double st;
    int res;
    
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, 0) < 0) {
        return -1;
    }
    
    if (descnonblock(sock) < 0) {
        return -1;
    }
    
    res = connect(sock, &msa.addr.generic, msa.addrlen);
    if (res == 0) {
        return 0;
    }
    
    if (errno != EINPROGRESS) {
        return -1;
    }
    
    st = monotonic_seconds();
    for (;;) {
        pfd.fd = sock;
        pfd.events = POLLOUT;
        pfd.revents = 0;
        
        res = poll(&pfd, 1, msecto);
        if (res > 0) {
            return tcpgetstatus(sock);
        }
        if (res == 0) {
            errno = ETIMEDOUT;
            return -1;
        }
        if (errno != EINTR) {
            return -1;
        }
        
        if (monotonic_seconds() - st >= msecto * 0.001) {
            errno = ETIMEDOUT;
            return -1;
        }
    }
}

int tcp6numlisten(int sock, const mfs_ip *ip, uint16_t port, uint16_t queue) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    if (bind(sock, &msa.addr.generic, msa.addrlen) < 0) {
        return -1;
    }
    return listen(sock, queue);
}

int tcp6strlisten(int sock, const char *hostname, const char *service, uint16_t queue) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, 1) < 0) {
        return -1;
    }
    if (bind(sock, &msa.addr.generic, msa.addrlen) < 0) {
        return -1;
    }
    return listen(sock, queue);
}

int tcp6getpeer(int sock, mfs_ip *ip, uint16_t *port) {
    mfs_sockaddr msa;
    msa.addrlen = sizeof(msa.addr);
    
    if (getpeername(sock, &msa.addr.generic, &msa.addrlen) < 0) {
        return -1;
    }
    
    if (msa.addr.generic.sa_family == AF_INET) {
        if (ip != NULL) {
            ip->family = AF_INET;
            ip->addr.v4 = ntohl(msa.addr.v4.sin_addr.s_addr);
        }
        if (port != NULL) {
            *port = ntohs(msa.addr.v4.sin_port);
        }
    } else if (msa.addr.generic.sa_family == AF_INET6) {
        if (ip != NULL) {
            ip->family = AF_INET6;
            memcpy(ip->addr.v6, &msa.addr.v6.sin6_addr, 16);
        }
        if (port != NULL) {
            *port = ntohs(msa.addr.v6.sin6_port);
        }
    } else {
        return -1;
    }
    
    return 0;
}

int tcp6getmyaddr(int sock, mfs_ip *ip, uint16_t *port) {
    mfs_sockaddr msa;
    msa.addrlen = sizeof(msa.addr);
    
    if (getsockname(sock, &msa.addr.generic, &msa.addrlen) < 0) {
        return -1;
    }
    
    if (msa.addr.generic.sa_family == AF_INET) {
        if (ip != NULL) {
            ip->family = AF_INET;
            ip->addr.v4 = ntohl(msa.addr.v4.sin_addr.s_addr);
        }
        if (port != NULL) {
            *port = ntohs(msa.addr.v4.sin_port);
        }
    } else if (msa.addr.generic.sa_family == AF_INET6) {
        if (ip != NULL) {
            ip->family = AF_INET6;
            memcpy(ip->addr.v6, &msa.addr.v6.sin6_addr, 16);
        }
        if (port != NULL) {
            *port = ntohs(msa.addr.v6.sin6_port);
        }
    } else {
        return -1;
    }
    
    return 0;
}

/* ----------------- UDP IPv6 ----------------- */

int udp6resolve(const char *hostname, const char *service, mfs_ip *ip, uint16_t *port, int passiveflag) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_DGRAM, passiveflag) < 0) {
        return -1;
    }
    
    if (ip != NULL) {
        if (msa.family == AF_INET) {
            ip->family = AF_INET;
            ip->addr.v4 = ntohl(msa.addr.v4.sin_addr.s_addr);
        } else if (msa.family == AF_INET6) {
            ip->family = AF_INET6;
            memcpy(ip->addr.v6, &msa.addr.v6.sin6_addr, 16);
        }
    }
    
    if (port != NULL) {
        if (msa.family == AF_INET) {
            *port = ntohs(msa.addr.v4.sin_port);
        } else if (msa.family == AF_INET6) {
            *port = ntohs(msa.addr.v6.sin6_port);
        }
    }
    
    return 0;
}

int udp6numlisten(int sock, const mfs_ip *ip, uint16_t port) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    return bind(sock, &msa.addr.generic, msa.addrlen);
}

int udp6strlisten(int sock, const char *hostname, const char *service) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_DGRAM, 1) < 0) {
        return -1;
    }
    return bind(sock, &msa.addr.generic, msa.addrlen);
}

int udp6write(int sock, const mfs_ip *ip, uint16_t port, const void *buff, uint16_t leng) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_fill(&msa, ip, port) < 0) {
        return -1;
    }
    return sendto(sock, buff, leng, 0, &msa.addr.generic, msa.addrlen);
}

int udp6read(int sock, mfs_ip *ip, uint16_t *port, void *buff, uint16_t leng) {
    mfs_sockaddr msa;
    int ret;
    
    msa.addrlen = sizeof(msa.addr);
    ret = recvfrom(sock, buff, leng, 0, &msa.addr.generic, &msa.addrlen);
    
    if (ret >= 0) {
        if (msa.addr.generic.sa_family == AF_INET) {
            if (ip != NULL) {
                ip->family = AF_INET;
                ip->addr.v4 = ntohl(msa.addr.v4.sin_addr.s_addr);
            }
            if (port != NULL) {
                *port = ntohs(msa.addr.v4.sin_port);
            }
        } else if (msa.addr.generic.sa_family == AF_INET6) {
            if (ip != NULL) {
                ip->family = AF_INET6;
                memcpy(ip->addr.v6, &msa.addr.v6.sin6_addr, 16);
            }
            if (port != NULL) {
                *port = ntohs(msa.addr.v6.sin6_port);
            }
        }
    }
    
    return ret;
}

/* -------------- Legacy Compatibility Wrappers -------------- */

#ifdef ENABLE_IPV6

int tcp6resolve_compat(const char *hostname, const char *service, uint32_t *ip, uint16_t *port, int passiveflag) {
    mfs_ip mip;
    int result = tcp6resolve(hostname, service, &mip, port, passiveflag);
    
    if (result == 0 && ip != NULL) {
        if (mip.family == AF_INET) {
            *ip = mip.addr.v4;
        } else {
            // For IPv6 addresses, we can't represent them in uint32_t
            // Return 0 (any address) to indicate IPv6 - caller should use IPv6 functions
            *ip = 0;
        }
    }
    
    return result;
}

int tcp6numlisten_compat(int sock, uint32_t ip, uint16_t port, uint16_t queue) {
    mfs_ip mip;
    
    if (ip == 0xFFFFFFFF) {
        // Special case: listen on all IPv6 interfaces
        mip.family = AF_INET6;
        memset(mip.addr.v6, 0, 16);
    } else {
        // IPv4 address
        mip.family = AF_INET;
        mip.addr.v4 = ip;
    }
    
    if (tcp6numbind(sock, &mip, port) < 0) {
        return -1;
    }
    
    return listen(sock, queue);
}

int tcp6numconnect_compat(int sock, uint32_t ip, uint16_t port) {
    mfs_ip mip;
    
    if (ip == 0) {
        // ip=0 from resolve indicates IPv6 - not supported in compat mode
        errno = EINVAL;
        return -1;
    }
    
    mip.family = AF_INET;
    mip.addr.v4 = ip;
    
    return tcp6numconnect(sock, &mip, port);
}

int tcp6getpeer_compat(int sock, uint32_t *ip, uint16_t *port) {
    mfs_ip mip;
    int result = tcp6getpeer(sock, &mip, port);
    
    if (result == 0 && ip != NULL) {
        if (mip.family == AF_INET) {
            *ip = mip.addr.v4;
        } else {
            // IPv6 address - use special value
            *ip = 0xFFFFFFFF;
        }
    }
    
    return result;
}

/* Universal socket functions that work with both IPv4 and IPv6 */

int univ_socket(void) {
    // Create an IPv6 socket that can handle both IPv4 and IPv6
    int sock = socket(AF_INET6, SOCK_STREAM, 0);
    if (sock >= 0) {
        // Disable IPv6-only mode to allow IPv4 connections too
        int no = 0;
        setsockopt(sock, IPPROTO_IPV6, IPV6_V6ONLY, &no, sizeof(no));
    }
    return sock;
}

int univ_resolve(const char *hostname, const char *service, struct sockaddr *addr, socklen_t *addrlen, int passiveflag) {
    mfs_sockaddr msa;
    if (mfs_sockaddr_resolve(&msa, hostname, service, SOCK_STREAM, passiveflag) < 0) {
        return -1;
    }
    
    if (msa.family == AF_INET) {
        memcpy(addr, &msa.addr.v4, sizeof(struct sockaddr_in));
        *addrlen = sizeof(struct sockaddr_in);
    } else if (msa.family == AF_INET6) {
        memcpy(addr, &msa.addr.v6, sizeof(struct sockaddr_in6));
        *addrlen = sizeof(struct sockaddr_in6);
    } else {
        return -1;
    }
    
    return 0;
}

int univ_listen(int sock, const struct sockaddr *addr, socklen_t addrlen, int queue) {
    if (bind(sock, addr, addrlen) < 0) {
        return -1;
    }
    return listen(sock, queue);
}

int univ_accept(int sock) {
    return accept(sock, NULL, NULL);
}

int univ_nonblock(int sock) {
    return descnonblock(sock);
}

int univ_nodelay(int sock) {
    int yes = 1;
    return setsockopt(sock, IPPROTO_TCP, TCP_NODELAY, &yes, sizeof(yes));
}

int univ_reuseaddr(int sock) {
    int yes = 1;
    return setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));
}

int univ_close(int sock) {
    return close(sock);
}

#endif /* ENABLE_IPV6 */