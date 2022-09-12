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
    os.remove('cow-diff')
except FileNotFoundError:
    pass


try:
    # Mount
    os.makedirs('cow', exist_ok=True)
    mount_proc = subprocess.Popen(['target/debug/cowblock', 'input.bin', 'cow'])
    time.sleep(2)
    assert mount_proc.returncode is None

    # Do some reads
    with open('cow/input.bin', 'rb') as fp:
        print('> read(4084, 24)', flush=True)
        fp.seek(4096 - 12, 0)
        data = fp.read(24)
        assert data == bytes(range(244, 256)) + bytes(range(0, 12)), data

        print('> read(4091, 4106)', flush=True)
        fp.seek(4096 - 5, 0)
        data = fp.read(4096 + 10)
        assert data[0:10] == b'\xFB\xFC\xFD\xFE\xFF\x00\x01\x02\x03\x04'
        assert data == bytes(range(251, 256)) + bytes(range(256)) * 16 + bytes(range(0, 5))

    # Do some writes
    with open('cow/input.bin', 'r+b') as fp:
        print('> write(3000, 3)', flush=True)
        fp.seek(3000, 0)
        fp.write(b'aaa')
        fp.flush()

        print('> write(4092, 4)', flush=True)
        fp.seek(4096 - 4)
        fp.write(b'cccccccc')
        fp.flush()

    # Read again
    with open('cow/input.bin', 'rb') as fp:
        print('> read(2999, 5)', flush=True)
        fp.seek(2999, 0)
        data = fp.read(5)
        assert data == b'\xB7aaa\xBB', data

        print('> read(4091, 10)', flush=True)
        fp.seek(4096 - 5, 0)
        data = fp.read(10)
        assert data == b'\xFBcccccccc\x04', data
finally:
    mount_proc.terminate()
    mount_proc.wait()
    subprocess.call(['fusermount', '-u', 'cow'])
