Block-Level Copy-on-Write
=========================

What is this?
-------------

This is a little tool allowing you to make changes to a big file without affecting the original, and without making an entire copy either.

It is implemented as a filesystem in userspace that keeps track of changes to the blocks of a file.

For example, if you have a big database or virtual hard drive and you want to make changes but keep the original; you can use this tool to create a copy of the file:

```
$ mkdir virtual_copy
$ cowblock my_big_file.sqlite3 virtual_copy &
$ sqlite3 virtual_copy/my_big_file.sqlite3
  # Make changes here
  # The changes are visible in virtual_copy/my_big_file.sqlite3,
  # but no change is made to my_big_file.sqlite3
  # The changed blocks are stored in virtual_copy-diff and virtual_code-extra,
  # which are much smaller files
$ fusermount -u virtual_copy/
```

Why?
----

There are options for copy-on-write filesystems, however they are quite limited:

* Tools like unionfs-fuse, aufs, and overlayfs do copy-on-write for filesystems, but copy whole files when you need to write to them. There is no way to copy part of a big file without makine a copy of it.
* Filesystems like btrfs or zfs offer copy-on-write (with `cp --reflink`), but you need to be using that filesystem to benefit from it.
* You can abuse devicemapper to do this but it is pretty difficult (and you'll get devices rather than regular files)

How?
----

The diff file contains an index at the start, indicating where the blocks of the file can be found in the diff. Blocks that haven't been overwritten yet are 0, while other numbers are the index of the block in the diff file.

The index is checked before every block read to determine whether we should read from the original or the diff.
