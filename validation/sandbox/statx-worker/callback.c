/* Diagnostic Linux/libfuse3 statx callback; not an adopted upstream API patch. */
static int op_statx(const char *path, int flags EXT2FS_ATTR((unused)),
                    int mask EXT2FS_ATTR((unused)), struct statx *sx,
                    struct fuse_file_info *fi)
{
    struct fuse2fs *ff = fuse_get_context()->private_data;
    ext2_filsys fs;
    ext2_ino_t ino;
    struct stat st = {0};
    struct timespec birth = {0};
    int has_birth = 0, ret;
    errcode_t err;
    FUSE2FS_CHECK_CONTEXT(ff);
    fs = ff->fs;
    if (fi && fi->fh) {
        struct fuse2fs_file_handle *fh =
            (struct fuse2fs_file_handle *)(uintptr_t)fi->fh;
        FUSE2FS_CHECK_MAGIC(fs, fh, FUSE2FS_FILE_MAGIC);
        ino = fh->ino;
    } else if (!path) {
        return -EBADF;
    }
    pthread_mutex_lock(&ff->bfl);
    if (!(fi && fi->fh)) {
        err = ext2fs_namei(fs, EXT2_ROOT_INO, EXT2_ROOT_INO, path, &ino);
        if (err) {
            ret = translate_error(fs, 0, err);
            goto out;
        }
    }
    ret = stat_inode_birth(fs, ino, &st, &birth, &has_birth);
    if (ret)
        goto out;
    memset(sx, 0, sizeof(*sx));
    sx->stx_mask = STATX_BASIC_STATS;
    sx->stx_blksize = st.st_blksize;
    sx->stx_nlink = st.st_nlink;
    sx->stx_uid = st.st_uid;
    sx->stx_gid = st.st_gid;
    sx->stx_mode = st.st_mode;
    sx->stx_ino = st.st_ino;
    sx->stx_size = st.st_size;
    sx->stx_blocks = st.st_blocks;
    sx->stx_dev_major = major(st.st_dev);
    sx->stx_dev_minor = minor(st.st_dev);
    sx->stx_rdev_major = major(st.st_rdev);
    sx->stx_rdev_minor = minor(st.st_rdev);
    sx->stx_atime.tv_sec = st.st_atim.tv_sec;
    sx->stx_atime.tv_nsec = st.st_atim.tv_nsec;
    sx->stx_mtime.tv_sec = st.st_mtim.tv_sec;
    sx->stx_mtime.tv_nsec = st.st_mtim.tv_nsec;
    sx->stx_ctime.tv_sec = st.st_ctim.tv_sec;
    sx->stx_ctime.tv_nsec = st.st_ctim.tv_nsec;
    if (has_birth) {
        sx->stx_mask |= STATX_BTIME;
        sx->stx_btime.tv_sec = birth.tv_sec;
        sx->stx_btime.tv_nsec = birth.tv_nsec;
    }
out:
    pthread_mutex_unlock(&ff->bfl);
    return ret;
}
