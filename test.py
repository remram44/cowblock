#!/usr/bin/env python3

import os
import subprocess
import time


# Make test input file
with open('input.bin', 'wb') as fp:
    for i in range(0, 4096 * 12, 256):
        fp.write(bytes(range(256)))


# Remove diff file
try:
    os.remove('diff.bin')
except FileNotFoundError:
    pass


try:
    # Mount
    os.makedirs('mount', exist_ok=True)
    mount_proc = subprocess.Popen(['target/debug/fuse-cow-block', 'input.bin', 'mount', 'diff.bin'])
    time.sleep(2)
    assert mount_proc.returncode is None

    # Do some reads
    with open('mount/input.bin', 'rb') as fp:
        fp.seek(4096 - 12, 0)
        data = fp.read(24)
        assert data == bytes(range(244, 256)) + bytes(range(0, 12)), data

        fp.seek(4096 - 12, 0)
        data = fp.read(4096 + 24)
        assert data == bytes(range(244, 256)) + bytes(range(256)) * 16 + bytes(range(0, 12)), data

    # Do some writes
    with open('mount/input.bin', 'r+b') as fp:
        fp.seek(3000, 0)
        fp.write(b'aaa')

        fp.seek(4096 - 4)
        fp.write(b'cccccccc')

    # Read again
    with open('mount/input.bin', 'rb') as fp:
        fp.seek(2999, 0)
        data = fp.read(5)
        assert data == b'oaaas', data

        fp.seek(4096 - 5, 0)
        data = fp.read(10)
        assert data == b'\xFBcccccccc\x04', data
finally:
    mount_proc.terminate()
    mount_proc.wait()
    subprocess.call(['fusermount', '-u', 'mount'])
