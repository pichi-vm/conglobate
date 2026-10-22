/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * Experimental fuse2fs ACL helpers. Not production conglobate code.
 * Linux xattr wire values are decoded with memcpy (no alignment assumptions).
 * libacl validates entry types, qualifiers, permissions and ACL structure.
 */
static int probe_acl_mode(const void *value, size_t len, mode_t *mode)
{
	const unsigned char *bytes = value;
	struct posix_acl_xattr_header header;
	struct posix_acl_xattr_entry wire;
	acl_t acl;
	acl_entry_t entry;
	acl_permset_t perms;
	size_t count, i;
	int ret = -EINVAL;

	if (len < sizeof(header) ||
	    (len - sizeof(header)) % sizeof(wire))
		return -EINVAL;
	memcpy(&header, bytes, sizeof(header));
	if (ext2fs_le32_to_cpu(header.a_version) != POSIX_ACL_XATTR_VERSION)
		return -EINVAL;
	count = (len - sizeof(header)) / sizeof(wire);
	if (count > INT_MAX)
		return -EINVAL;
	acl = acl_init((int)count);
	if (!acl)
		return -errno;
	for (i = 0; i < count; i++) {
		unsigned tag, bits;
		id_t id;

		memcpy(&wire, bytes + sizeof(header) + i * sizeof(wire), sizeof(wire));
		tag = ext2fs_le16_to_cpu(wire.e_tag);
		bits = ext2fs_le16_to_cpu(wire.e_perm);
		id = ext2fs_le32_to_cpu(wire.e_id);
		if (bits & ~7U)
			goto out;
		if (acl_create_entry(&acl, &entry) ||
		    acl_set_tag_type(entry, tag) ||
		    acl_get_permset(entry, &perms) || acl_clear_perms(perms))
			goto out;
		if ((bits & 4) && acl_add_perm(perms, ACL_READ))
			goto out;
		if ((bits & 2) && acl_add_perm(perms, ACL_WRITE))
			goto out;
		if ((bits & 1) && acl_add_perm(perms, ACL_EXECUTE))
			goto out;
		if ((tag == ACL_USER || tag == ACL_GROUP) &&
		    acl_set_qualifier(entry, &id))
			goto out;
	}
	if (acl_valid(acl))
		goto out;
	if (acl_equiv_mode(acl, mode) < 0)
		goto out;
	ret = 0;
out:
	acl_free(acl);
	return ret;
}

/* Intersect class permissions on create; replace them on chmod. */
static int probe_acl_adjust(void *value, size_t len, mode_t mode, int intersect,
			    mode_t *result)
{
	unsigned char *bytes = value;
	struct posix_acl_xattr_entry wire;
	size_t count, i, offset;
	int has_mask = 0, ret;
	mode_t ignored;

	ret = probe_acl_mode(value, len, &ignored);
	if (ret)
		return ret;
	count = (len - sizeof(struct posix_acl_xattr_header)) / sizeof(wire);
	for (i = 0; i < count; i++) {
		offset = sizeof(struct posix_acl_xattr_header) + i * sizeof(wire);
		memcpy(&wire, bytes + offset, sizeof(wire));
		has_mask |= ext2fs_le16_to_cpu(wire.e_tag) == ACL_MASK;
	}
	for (i = 0; i < count; i++) {
		unsigned tag, wanted, old;
		offset = sizeof(struct posix_acl_xattr_header) + i * sizeof(wire);
		memcpy(&wire, bytes + offset, sizeof(wire));
		tag = ext2fs_le16_to_cpu(wire.e_tag);
		old = ext2fs_le16_to_cpu(wire.e_perm);
		if (tag == ACL_USER_OBJ)
			wanted = (mode >> 6) & 7;
		else if (tag == ACL_OTHER)
			wanted = mode & 7;
		else if (tag == ACL_MASK || (tag == ACL_GROUP_OBJ && !has_mask))
			wanted = (mode >> 3) & 7;
		else
			continue;
		wire.e_perm = ext2fs_cpu_to_le16(intersect ? old & wanted : wanted);
		memcpy(bytes + offset, &wire, sizeof(wire));
	}
	return probe_acl_mode(value, len, result);
}

static int in_file_group(struct fuse_context *ctxt,
			 const struct ext2_inode_large *inode);

static int probe_acl_inode_mode(struct fuse2fs *ff, ext2_ino_t ino, mode_t mode,
			       int setting_acl)
{
	struct ext2_inode_large inode;
	errcode_t err;
	int ret;

	/* Read after xattr updates, which may change the inode's xattr pointers. */
	err = fuse2fs_read_inode(ff->fs, ino, &inode);
	if (err)
		return translate_error(ff->fs, ino, err);
	inode.i_mode = (inode.i_mode & ~0777) | (mode & 0777);
	if (setting_acl && !is_superuser(ff, fuse_get_context())) {
		ret = in_file_group(fuse_get_context(), &inode);
		if (ret < 0)
			return ret;
		if (!ret)
			inode.i_mode &= ~S_ISGID;
	}
	ret = update_ctime(ff->fs, ino, &inode);
	if (ret)
		return ret;
	err = fuse2fs_write_inode(ff->fs, ino, &inode);
	return err ? translate_error(ff->fs, ino, err) : 0;
}

static int probe_acl_chmod(struct fuse2fs *ff, ext2_ino_t ino, mode_t mode)
{
	void *value = NULL;
	size_t len;
	mode_t result;
	int ret;

	if (!ff->acl)
		return 0;
	ret = __getxattr(ff, ino, XATTR_NAME_POSIX_ACL_ACCESS, &value, &len);
	if (ret == -ENODATA || ret == -ENOENT)
		return 0;
	if (ret)
		return ret;
	ret = probe_acl_adjust(value, len, mode, 0, &result);
	if (!ret)
		ret = __setxattr(ff, ino, XATTR_NAME_POSIX_ACL_ACCESS, value, len);
	ext2fs_free_mem(&value);
	return ret;
}

static int propagate_default_acls(struct fuse2fs *ff, ext2_ino_t parent,
				  ext2_ino_t child)
{
	struct ext2_inode_large inode;
	void *value = NULL;
	size_t len;
	mode_t result;
	errcode_t err;
	int ret;

	if (!ff->acl)
		return 0;
	err = fuse2fs_read_inode(ff->fs, child, &inode);
	if (err)
		return translate_error(ff->fs, child, err);
	ret = __getxattr(ff, parent, XATTR_NAME_POSIX_ACL_DEFAULT, &value, &len);
	if (ret == -ENODATA || ret == -ENOENT)
		return probe_acl_inode_mode(ff, child,
				inode.i_mode & ~fuse_get_context()->umask, 0);
	if (ret)
		return ret;
	ret = probe_acl_mode(value, len, &result);
	if (ret)
		goto out;
	if (S_ISDIR(inode.i_mode)) {
		ret = __setxattr(ff, child, XATTR_NAME_POSIX_ACL_DEFAULT, value, len);
		if (ret)
			goto out;
	}
	ret = probe_acl_adjust(value, len, inode.i_mode, 1, &result);
	if (!ret && len != sizeof(struct posix_acl_xattr_header) +
			  3 * sizeof(struct posix_acl_xattr_entry))
		ret = __setxattr(ff, child, XATTR_NAME_POSIX_ACL_ACCESS, value, len);
	if (!ret)
		ret = probe_acl_inode_mode(ff, child, result, 0);
out:
	ext2fs_free_mem(&value);
	return ret;
}
