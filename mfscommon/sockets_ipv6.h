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

#ifndef _SOCKETS_IPV6_H_
#define _SOCKETS_IPV6_H_

#include <inttypes.h>
#include <errno.h>
#include <sys/socket.h>
#include <netinet/in.h>

/* IPv6 Address Structure */
typedef struct mfs_sockaddr {
    int family;  // AF_INET or AF_INET6
    union {
        struct sockaddr_in v4;
        struct sockaddr_in6 v6;
        struct sockaddr generic;
    } addr;
    socklen_t addrlen;
} mfs_sockaddr;

/* String sizes for IPv6 */
#define STRIPSIZE_V6 46       // INET6_ADDRSTRLEN
#define STRIPPORTSIZE_V6 58   // [xxxx:xxxx:xxxx:xxxx:xxxx:xxxx:xxxx:xxxx]:65535

/* Universal IP representation */
typedef struct mfs_ip {
    int family;
    union {
        uint32_t v4;
        uint8_t v6[16];
    } addr;
} mfs_ip;

/* -------------- IPv6 UNIVERSAL -------------- */

void univ6makestrip(char strip[STRIPSIZE_V6], const mfs_ip *ip);
void univ6makestripport(char stripport[STRIPPORTSIZE_V6], const mfs_ip *ip, uint16_t port);
char* univ6allocstrip(const mfs_ip *ip);
char* univ6allocstripport(const mfs_ip *ip, uint16_t port);

/* Convert between old and new IP formats */
void ipv4_to_mfs_ip(mfs_ip *mip, uint32_t ipv4);
uint32_t mfs_ip_to_ipv4(const mfs_ip *mip);
int mfs_ip_is_v4(const mfs_ip *mip);
int mfs_ip_is_v6(const mfs_ip *mip);
int mfs_ip_equal(const mfs_ip *a, const mfs_ip *b);

/* ----------------- TCP IPv6 ----------------- */

int tcp6socket(void);
int tcp6resolve(const char *hostname, const char *service, mfs_ip *ip, uint16_t *port, int passiveflag);
int tcp6numbind(int sock, const mfs_ip *ip, uint16_t port);
int tcp6strbind(int sock, const char *hostname, const char *service);
int tcp6numconnect(int sock, const mfs_ip *ip, uint16_t port);
int tcp6numtoconnect(int sock, const mfs_ip *ip, uint16_t port, uint32_t msecto);
int tcp6strconnect(int sock, const char *hostname, const char *service);
int tcp6strtoconnect(int sock, const char *hostname, const char *service, uint32_t msecto);
int tcp6numlisten(int sock, const mfs_ip *ip, uint16_t port, uint16_t queue);
int tcp6strlisten(int sock, const char *hostname, const char *service, uint16_t queue);
int tcp6getpeer(int sock, mfs_ip *ip, uint16_t *port);
int tcp6getmyaddr(int sock, mfs_ip *ip, uint16_t *port);

/* ----------------- UDP IPv6 ----------------- */

int udp6socket(void);
int udp6resolve(const char *hostname, const char *service, mfs_ip *ip, uint16_t *port, int passiveflag);
int udp6numlisten(int sock, const mfs_ip *ip, uint16_t port);
int udp6strlisten(int sock, const char *hostname, const char *service);
int udp6write(int sock, const mfs_ip *ip, uint16_t port, const void *buff, uint16_t leng);
int udp6read(int sock, mfs_ip *ip, uint16_t *port, void *buff, uint16_t leng);

/* Compatibility macros for gradual migration */
#ifdef ENABLE_IPV6
  #define MFS_TCP_SOCKET() tcp6socket()
  #define MFS_UDP_SOCKET() udp6socket()
  
  /* Legacy compatibility layer - redirect old functions to IPv6 versions */
  #define tcpsocket() tcp6socket()
  #define udpsocket() udp6socket()
  #define tcpresolve(hostname, service, ip, port, passive) \
    tcp6resolve_compat(hostname, service, ip, port, passive)
  #define tcpnumlisten(sock, ip, port, queue) \
    tcp6numlisten_compat(sock, ip, port, queue)
  #define tcpstrlisten(sock, hostname, service, queue) \
    tcp6strlisten(sock, hostname, service, queue)
  #define tcpnumconnect(sock, ip, port) \
    tcp6numconnect_compat(sock, ip, port)
  #define tcpstrconnect(sock, hostname, service) \
    tcp6strconnect(sock, hostname, service)
  #define tcpgetpeer(sock, ip, port) \
    tcp6getpeer_compat(sock, ip, port)
#else
  #define MFS_TCP_SOCKET() tcpsocket()
  #define MFS_UDP_SOCKET() udpsocket()
#endif

/* Compatibility wrapper functions for legacy uint32_t IP addresses */
#ifdef ENABLE_IPV6
int tcp6resolve_compat(const char *hostname, const char *service, uint32_t *ip, uint16_t *port, int passiveflag);
int tcp6numlisten_compat(int sock, uint32_t ip, uint16_t port, uint16_t queue);
int tcp6numconnect_compat(int sock, uint32_t ip, uint16_t port);
int tcp6getpeer_compat(int sock, uint32_t *ip, uint16_t *port);

// Universal socket functions that work with both IPv4 and IPv6
int univ_socket(void);
int univ_resolve(const char *hostname, const char *service, struct sockaddr *addr, socklen_t *addrlen, int passiveflag);
int univ_listen(int sock, const struct sockaddr *addr, socklen_t addrlen, int queue);
int univ_accept(int sock);
int univ_nonblock(int sock);
int univ_nodelay(int sock);
int univ_reuseaddr(int sock);
int univ_close(int sock);
#endif

#endif /* _SOCKETS_IPV6_H_ */