/*
 * Copyright (C) 2025 Jakub Kruszona-Zawadzki, Saglabs SA
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

#ifndef _FILESYSTEM_H_
#define _FILESYSTEM_H_

#include <stdio.h>
#include <inttypes.h>
#include "bio.h"
#include "MFSCommunication.h"

uint8_t fs_mr_access(uint32_t ts,uint32_t inode);
uint8_t fs_mr_append_slice(uint32_t ts,uint32_t inode,uint32_t inode_src,uint32_t slice_from,uint32_t slice_to);
uint8_t fs_mr_acquire(uint32_t inode,uint32_t sessionid);
uint8_t fs_mr_attr(uint32_t ts,uint32_t inode,uint16_t mode,uint32_t uid,uint32_t gid,uint32_t atime,uint32_t mtime,uint8_t winattr,uint16_t attrmode);
uint8_t fs_mr_amtime(uint32_t inode,uint32_t ts,uint32_t atime,uint32_t mtime,uint32_t ctime);
// int fs_copy(uint32_t ts,inode,parent,strlen(name),name);
uint8_t fs_mr_create(uint32_t ts,uint32_t parent,uint32_t nleng,const uint8_t *name,uint8_t type,uint16_t mode,uint16_t cumask,uint32_t uid,uint32_t gid,uint32_t rdev,uint32_t inode);
uint8_t fs_mr_session(uint32_t sessionid);
uint8_t fs_mr_emptytrash(uint32_t ts,uint32_t bid,uint32_t freeinodes,uint32_t sustainedinodes,uint32_t trashflaginodes,uint32_t inode_chksum);
uint8_t fs_mr_emptysustained(uint32_t ts,uint32_t bid,uint32_t freeinodes,uint32_t inode_chksum);
uint8_t fs_mr_freeinodes(uint32_t ts,uint32_t inodereusedelay,uint32_t freeinodes,uint32_t sustainedinodes,uint32_t inode_chksum);
uint8_t fs_mr_link(uint32_t ts,uint32_t inode_src,uint32_t parent_dst,uint32_t nleng_dst,uint8_t *name_dst);
uint8_t fs_mr_length(uint32_t ts,uint32_t inode,uint64_t length,uint8_t canmodmtime);
uint8_t fs_mr_move(uint32_t ts,uint32_t parent_src,uint32_t nleng_src,const uint8_t *name_src,uint32_t parent_dst,uint32_t nleng_dst,const uint8_t *name_dst,uint8_t rmode,uint32_t inode);
uint8_t fs_mr_repair(uint32_t ts,uint32_t inode,uint32_t indx,uint32_t nversion);
// uint8_t fs_reinit(uint32_t ts,uint32_t inode,uint32_t indx,uint64_t chunkid);
uint8_t fs_mr_release(uint32_t inode,uint32_t sessionid);
uint8_t fs_mr_symlink(uint32_t ts,uint32_t parent,uint32_t nleng,const uint8_t *name,const uint8_t *path,uint32_t uid,uint32_t gid,uint32_t inode);
uint8_t fs_mr_setpath(uint32_t inode,const uint8_t *path);
uint8_t fs_mr_snapshot(uint32_t ts,uint32_t inode_src,uint32_t parent_dst,uint16_t nleng_dst,uint8_t *name_dst,uint8_t smode,uint8_t sesflags,uint32_t uid,uint32_t gids,uint32_t *gid,uint16_t cumask,uint32_t inodecheck,uint32_t removed,uint32_t same,uint32_t existing,uint32_t hardlinks,uint32_t new);
uint8_t fs_mr_unlink(uint32_t ts,uint32_t parent,uint32_t nleng,const uint8_t *name,uint32_t inode);
uint8_t fs_mr_trash_recover(uint32_t ts,uint32_t inode,uint32_t parent_dst,uint32_t pleng_dst,const uint8_t *path_dst,uint16_t cumask,uint32_t uid,uint32_t gid,uint8_t copysgid,uint32_t created_pleng,const uint8_t *created_path);
uint8_t fs_mr_trash_remove(uint32_t ts,uint32_t inode);
uint8_t fs_mr_purge(uint32_t ts,uint32_t inode);
uint8_t fs_mr_undel(uint32_t ts,uint32_t inode);
uint8_t fs_mr_trunc(uint32_t ts,uint32_t inode,uint32_t indx,uint64_t chunkid);
uint8_t fs_mr_write(uint32_t ts,uint32_t inode,uint32_t indx,uint8_t opflag,uint8_t canmodmtime,uint64_t chunkid);
uint8_t fs_mr_rollback(uint32_t ts,uint32_t inode,uint32_t indx,uint64_t prevchunkid,uint64_t chunkid);
uint8_t fs_mr_unlock(uint32_t ts,uint64_t chunkid);
uint8_t fs_mr_setsclass(uint32_t ts,uint32_t inode,uint32_t uid,uint8_t src_sclassid,uint8_t dst_sclassid,uint8_t smode,uint32_t sinodes,uint32_t ncinodes,uint32_t nsinodes);
uint8_t fs_mr_settrashretention(uint32_t ts,uint32_t inode,uint32_t uid,uint32_t trashretention,uint8_t smode,uint32_t sinodes,uint32_t ncinodes,uint32_t nsinodes);
uint8_t fs_mr_seteattr(uint32_t ts,uint32_t inode,uint32_t uid,uint8_t eattr,uint8_t smode,uint32_t sinodes,uint32_t ncinodes,uint32_t nsinodes);
uint8_t fs_mr_setxattr(uint32_t ts,uint32_t inode,uint32_t anleng,const uint8_t *attrname,uint32_t avleng,const uint8_t *attrvalue,uint32_t mode);
uint8_t fs_mr_setacl(uint32_t ts,uint32_t inode,uint16_t mode,uint8_t changectime,uint8_t acltype,uint16_t userperm,uint16_t groupperm,uint16_t otherperm,uint16_t mask,uint16_t namedusers,uint16_t namedgroups,const uint8_t *aclblob);
uint8_t fs_mr_quota(uint32_t ts,uint32_t inode,uint8_t exceeded,uint8_t flags,uint32_t stimestamp,uint32_t sinodes,uint32_t hinodes,uint64_t slength,uint64_t hlength,uint64_t ssize,uint64_t hsize,uint64_t srealsize,uint64_t hrealsize,uint32_t graceperiod);
uint8_t fs_mr_archchg(uint32_t ts,uint32_t inode,uint32_t uid,uint8_t flags,uint64_t chgchunks,uint64_t notchgchunks,uint32_t nsinodes);
uint8_t fs_mr_set_file_chunk(uint32_t inode,uint32_t indx,uint64_t chunkdid);
uint8_t fs_mr_autoarch(uint32_t inode,uint32_t archreftime,uint8_t intrash,uint32_t archchgchunks,uint32_t trashchgchunks);
uint8_t fs_mr_additionalattr(uint32_t ts,uint32_t inode,uint8_t flags,const uint8_t *data,uint32_t leng);


//uint64_t fs_mr_getversion(void);

void fs_text_dump(FILE *fd);

void fs_stats(uint32_t stats[24]);
void fs_info(uint64_t *totalspace,uint64_t *availspace,uint64_t *freespace,uint64_t *trspace,uint32_t *trnodes,uint64_t *respace,uint32_t *renodes,uint32_t *inodes,uint32_t *dnodes,uint32_t *fnodes);
void fs_charts_data(uint32_t *file_objects,uint32_t *meta_objects);
void fs_test_getdata(uint32_t *loopstart,uint32_t *loopend,uint32_t *files,uint32_t *ugfiles,uint32_t *mfiles,uint32_t *mtfiles,uint32_t *msfiles,uint32_t *chunks,uint32_t *ugchunks,uint32_t *mchunks,char **msgbuff,uint32_t *msgbuffleng);

// void fs_attrtoblob(uint8_t attr[32],uint8_t attrblob[32]);

uint32_t fs_getdirpath_size(uint32_t inode);
void fs_getdirpath_data(uint32_t inode,uint8_t *buff,uint32_t size);
uint8_t fs_getrootinode(uint32_t *rootinode,const uint8_t *path);

uint8_t fs_path_lookup(uint32_t rootinode,uint8_t sesflags,uint32_t base_inode,uint32_t pleng,const uint8_t *path,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint32_t *parent_inode,uint32_t *last_inode,uint8_t *nleng,uint8_t name[MFS_NAME_MAX],uint8_t attr[ATTR_RECORD_SIZE]);
void fs_statfs(uint32_t rootinode,uint8_t sesflags,uint64_t *totalspace,uint64_t *availspace,uint64_t *freespace,uint64_t *trashspace,uint64_t *sustainedspace,uint32_t *inodes);
uint8_t fs_access(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint32_t gids,uint32_t *gid,int modemask);
uint8_t fs_lookup(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE],uint8_t allow_recover,uint16_t *accmode,uint8_t *filenode,uint8_t *validchunk,uint64_t *chunkid);
uint8_t fs_getattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gid,uint32_t auid,uint32_t agid,uint8_t attr[ATTR_RECORD_SIZE]);
uint8_t fs_setattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint8_t setmask,uint16_t attrmode,uint32_t attruid,uint32_t attrgid,uint32_t attratime,uint32_t attrmtime,uint8_t winattr,uint8_t sugidclearmode,uint8_t attr[ATTR_RECORD_SIZE]);

uint8_t fs_set_additional_attributes(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t flags,uint32_t uid,const uint8_t *data,uint32_t leng);

uint8_t fs_try_setlength(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t flags,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t disflags,uint64_t length,uint32_t *indx,uint64_t *prevchunkid,uint64_t *chunkid);
// uint8_t fs_try_setlength(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint64_t length,uint8_t attr[ATTR_RECORD_SIZE],uint64_t *chunkid);
uint8_t fs_end_setlength(uint64_t chunkid);
uint8_t fs_do_setlength(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t flags,uint32_t uid,uint32_t gid,uint32_t auid,uint32_t agid,uint64_t length,uint8_t attr[ATTR_RECORD_SIZE],uint64_t *prevlength);

uint8_t fs_readlink(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t *pleng,uint8_t **path);
uint8_t fs_symlink(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint32_t pleng,const uint8_t *path,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE]);
uint8_t fs_mknod(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint8_t type,uint16_t mode,uint16_t cumask,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint32_t rdev,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE],uint8_t *oflags);
uint8_t fs_mkdir(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint16_t mode,uint16_t cumask,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint8_t copysgid,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE]);
uint8_t fs_unlink(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t *inode);
uint8_t fs_rmdir(uint32_t rootinode,uint8_t sesflags,uint32_t parent,uint16_t nleng,const uint8_t *name,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t *inode);
uint8_t fs_rename(uint32_t rootinode,uint8_t sesflags,uint32_t parent_src,uint16_t nleng_src,const uint8_t *name_src,uint32_t parent_dst,uint16_t nleng_dst,const uint8_t *name_dst,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint8_t rmode,uint8_t delflags,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE]);
uint8_t fs_link(uint32_t rootinode,uint8_t sesflags,uint32_t inode_src,uint32_t parent_dst,uint16_t nleng_dst,const uint8_t *name_dst,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint32_t *inode,uint8_t attr[ATTR_RECORD_SIZE]);
uint8_t fs_snapshot(uint32_t rootinode,uint8_t sesflags,uint32_t inode_src,uint32_t parent_dst,uint16_t nleng_dst,const uint8_t *name_dst,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t smode,uint16_t requmask);
uint8_t fs_append_slice(uint32_t rootinode,uint8_t sesflags,uint8_t flags,uint32_t inode,uint32_t inode_src,uint32_t slice_from,uint32_t slice_to,uint32_t uid,uint32_t gids,uint32_t *gid,uint64_t *fleng);

uint8_t fs_readdir_size(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t flags,uint32_t maxentries,uint64_t nedgeid,void **dnode,void **dedge,uint32_t *dbuffsize,uint8_t attrmode);
void fs_readdir_data(uint32_t rootinode,uint8_t sesflags,uint32_t uid,uint32_t gid,uint32_t auid,uint32_t agid,uint8_t flags,uint32_t maxentries,uint64_t *nedgeid,void *dnode,void *dedge,uint8_t *dbuff,uint8_t attrmode);

uint8_t fs_filechunk(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t indx,uint64_t *chunkid);
uint8_t fs_checkfile(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t mode,uint32_t chunkcount[2774]);

uint8_t fs_opencheck(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint32_t gids,uint32_t *gid,uint32_t auid,uint32_t agid,uint8_t flags,uint8_t attr[ATTR_RECORD_SIZE],uint8_t *oflags);

uint8_t fs_readchunk(uint32_t inode,uint32_t sesflags,uint32_t indx,uint8_t chunkopflags,uint8_t allow_recover,uint64_t *chunkid,uint64_t *length);
// uint8_t fs_writechunk(uint32_t inode,uint32_t indx,uint64_t *chunkid,uint64_t *length,uint8_t *opflag);
uint8_t fs_writechunk(uint32_t inode,uint32_t indx,uint8_t chunkopflags,uint64_t *prevchunkid,uint64_t *chunkid,uint64_t *length,uint8_t *opflag,uint32_t clientip);
// uint8_t fs_reinitchunk(uint32_t inode,uint32_t indx,uint64_t *chunkid);
uint8_t fs_writeend(uint32_t inode,uint64_t length,uint64_t chunkid,uint8_t chunkopflags,uint8_t *flenghaschanged);

uint8_t fs_rollback(uint32_t inode,uint32_t indx,uint64_t prevchunkid,uint64_t chunkid);

uint8_t fs_repair(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t flags,uint32_t *notchanged,uint32_t *erased,uint32_t *repaired);

void fs_amtime_update(uint32_t rootinode,uint8_t sesflags,uint32_t *inodetab,uint32_t *atimetab,uint32_t *mtimetab,uint32_t cnt);

uint8_t fs_getsclass(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t gmode,uint32_t fgtab[MAXSCLASS],uint32_t dgtab[MAXSCLASS]);
uint8_t fs_setsclass(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint8_t src_sclassid,uint8_t dst_sclassid,uint8_t smode,uint32_t *sinodes,uint32_t *ncinodes,uint32_t *nsinodes);

uint8_t fs_gettrashretention_prepare(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t gmode,void **fptr,void **dptr,uint32_t *fnodes,uint32_t *dnodes);
void fs_gettrashretention_store(void *fptr,void *dptr,uint8_t *buff);
uint8_t fs_settrashretention(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint32_t trashretention,uint8_t smode,uint32_t *sinodes,uint32_t *ncinodes,uint32_t *nsinodes);

uint8_t fs_geteattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t gmode,uint32_t feattrtab[1<<EATTR_BITS],uint32_t deattrtab[1<<EATTR_BITS]);
uint8_t fs_seteattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint8_t eattr,uint8_t smode,uint32_t *sinodes,uint32_t *ncinodes,uint32_t *nsinodes);

uint8_t fs_listxattr_leng(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,void **xanode,uint32_t *xasize);
void fs_listxattr_data(void *xanode,uint8_t *xabuff);
uint8_t fs_setxattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t anleng,const uint8_t *attrname,uint32_t avleng,const uint8_t *attrvalue,uint8_t mode);
uint8_t fs_getxattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t anleng,const uint8_t *attrname,uint32_t *avleng,const uint8_t **attrvalue);

uint8_t fs_setfacl(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint8_t acltype,uint16_t userperm,uint16_t groupperm,uint16_t otherperm,uint16_t mask,uint16_t namedusers,uint16_t namedgroups,const uint8_t *aclblob);
uint8_t fs_getfacl_size(uint32_t rootinode,uint8_t sesflags,uint32_t inode,/*uint8_t opened,uint32_t uid,uint32_t gids,uint32_t *gid,*/uint8_t acltype,void **custom,uint32_t *aclblobsize);
void fs_getfacl_data(void *custom,uint16_t *userperm,uint16_t *groupperm,uint16_t *otherperm,uint16_t *mask,uint16_t *namedusers,uint16_t *namedgroups,uint8_t *aclblob);

uint8_t fs_get_parents_count(uint32_t rootinode,uint32_t inode,uint32_t *cnt);
void fs_get_parents_data(uint32_t rootinode,uint32_t inode,uint8_t *buff);
uint8_t fs_get_paths_size(uint32_t rootinode,uint32_t inode,uint32_t *psize);
void fs_get_paths_data(uint32_t rootinode,uint32_t inode,uint8_t *buff);

uint8_t fs_archget(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint64_t *archchunks,uint64_t *notarchchunks,uint32_t *archinodes,uint32_t *partinodes,uint32_t *notarchinodes);
uint8_t fs_archchg(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t uid,uint8_t cmd,uint64_t *chgchunks,uint64_t *notchgchunks,uint32_t *nsinodes);

uint32_t fs_node_info(uint32_t rootinode,uint8_t sesflags,uint8_t eights_mode,uint32_t inode,uint32_t maxentries,uint64_t continueid,uint8_t *ptr);
uint32_t fs_chunk_info(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t indx,uint16_t maxentries,uint8_t *ptr);
uint8_t fs_readdirfull(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t flags,uint32_t maxentries,uint64_t *nedgeidp,void **dedge,uint8_t *dbuff,uint32_t *dbuffsize);

// TRASH+SUSTAINED via TOOLS
uint8_t fs_trash_recover(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t parent_dst,uint32_t pleng_dst,const uint8_t *path_dst,uint16_t cumask,uint32_t uid,uint32_t gids,uint32_t *gid,uint8_t copysgid,uint32_t *used_pleng,uint8_t used_path[MFS_PATH_MAX]);
uint8_t fs_trash_remove(uint8_t sesflags,uint32_t inode,uint32_t uid);

// SUSTAINED
// - for 'meta' in mfsmount
uint8_t fs_readsustained_size(uint32_t rootinode,uint8_t sesflags,uint32_t *dbuffsize);
void fs_readsustained_data(uint32_t rootinode,uint8_t sesflags,uint8_t *dbuff);
// - for mfstools
uint32_t fs_listsustained(uint8_t partno,uint32_t uid,uint8_t gnleng,const uint8_t *gname,uint8_t *dbuff);

// TRASH
// - for 'meta' in mfsmount
uint8_t fs_readtrash_size(uint32_t rootinode,uint8_t sesflags,uint32_t tid,uint32_t *dbuffsize);
void fs_readtrash_data(uint32_t rootinode,uint8_t sesflags,uint32_t tid,uint8_t *dbuff);
uint8_t fs_gettrashpath(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t *pleng,const uint8_t **path);
uint8_t fs_settrashpath(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t pleng,const uint8_t *path);
uint8_t fs_purge(uint32_t rootinode,uint8_t sesflags,uint32_t inode);
uint8_t fs_undel(uint32_t rootinode,uint8_t sesflags,uint32_t inode);
// - for mfstools
uint32_t fs_listtrash(uint8_t partno,uint8_t format,uint32_t uid,uint32_t mints,uint32_t maxts,uint8_t gnleng,const uint8_t *gname,uint8_t *dbuff);

// SUSTAINED+TRASH (for 'meta' in mfsmount)
uint8_t fs_getdetachedattr(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t attr[ATTR_RECORD_SIZE],uint8_t dtype);

// EXTRA
uint8_t fs_get_dir_stats(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint32_t *inodes,uint32_t *dirs,uint32_t *files,uint32_t *chunks,uint64_t *length,uint64_t *size,uint64_t *rsize);

// QUOTA
uint8_t fs_quotacontrol(uint32_t rootinode,uint8_t sesflags,uint32_t inode,uint8_t delflag,uint8_t *flags,uint8_t *defaultgp,uint32_t *graceperiod,uint32_t *sinodes,uint64_t *slength,uint64_t *ssize,uint64_t *srealsize,uint32_t *hinodes,uint64_t *hlength,uint64_t *hsize,uint64_t *hrealsize,uint32_t *curinodes,uint64_t *curlength,uint64_t *cursize,uint64_t *currealsize);
//uint8_t fs_deletequota(uint32_t inode,uint8_t sflags,uint8_t hflags);
//uint8_t fs_setquota(uint32_t inode,uint8_t sflags,uint8_t hflags,uint32_t sinodes,uint64_t slength,uint64_t ssize,uint64_t srealsize,uint32_t hinodes,uint64_t hlength,uint64_t hsize,uint64_t hrealsize);
//uint8_t fs_getquota(uint32_t inode,uint8_t *sflags,uint8_t *hflags,uint32_t *sinodes,uint64_t *slength,uint64_t *ssize,uint64_t *srealsize,uint32_t *hinodes,uint64_t *hlength,uint64_t *hsize,uint64_t *hrealsize);
uint32_t fs_getquotainfo(uint8_t *buff,uint8_t ver);

// SPECIAL - LOG EMERGENCY INCREASE VERSION FROM CHUNKS-MODULE
void fs_incversion(uint64_t chunkid);

int fs_set_root_times(uint32_t ts);

void fs_new(void);

void fs_get_memusage(uint64_t allocated[8],uint64_t used[8]);

void fs_cleanup(void);
void fs_printinfo(void);
void fs_afterload(void);
int fs_check_consistency(int ignoreflag);

int fs_importnodes(bio *fd,uint32_t maxnodeid,int ignoreflag);

int fs_loadnodes(bio *fd,uint8_t mver,int ignoreflag);
int fs_loadedges(bio *fd,uint8_t mver,int ignoreflag);
int fs_loadfree(bio *fd,uint8_t mver,int ignoreflag);
int fs_loadquota(bio *fd,uint8_t mver,int ignoreflag);
uint8_t fs_storenodes(bio *fd);
uint8_t fs_storeedges(bio *fd);
uint8_t fs_storefree(bio *fd);
uint8_t fs_storequota(bio *fd);

uint8_t fs_mr_renumerate_edges(uint64_t expected_nextedgeid);
void fs_renumerate_edge_test(void);

int fs_strinit(void);

uint8_t fs_check_inode(uint32_t inode);

void fs_set_xattrflag(uint32_t inode);
void fs_del_xattrflag(uint32_t inode);

void fs_set_aclflag(uint32_t inode,uint8_t acltype);
void fs_del_aclflag(uint32_t inode,uint8_t acltype);
uint16_t fs_get_mode(uint32_t inode);

#endif
