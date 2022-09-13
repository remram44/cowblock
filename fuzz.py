#!/usr/bin/env python3

import os
import random
import struct
import subprocess
import time


# Make test data
def make_data(size, seed):
    rand = random.Random(seed)
    return bytes(rand.randint(0, 255) for _ in range(size))


def hexstring(data):
    lines = []
    for i in range(0, len(data), 16):
        s = data[i:i + 16]
        lines.append(' '.join('%02X' % b for b in s))
    return '\n'.join(lines)


def do_test(seed):
    print("\ndo_test(seed=%d)" % seed, flush=True)

    # Make data array
    rand = random.Random(seed)
    init_data_size = rand.randint(30, 600)
    data = make_data(
        init_data_size,
        rand.randint(0, (1 << 32) - 1),
    )

    print('> make_data(%d)' % len(data), flush=True)

    # Make test input file
    with open('input.bin', 'wb') as fp:
        fp.write(data)
    assert os.path.getsize('input.bin') == len(data)

    # Remove diff and extra files
    try:
        os.remove('cow-diff')
    except FileNotFoundError:
        pass
    try:
        os.remove('cow-extra')
    except FileNotFoundError:
        pass

    try:
        # Mount
        os.makedirs('cow', exist_ok=True)
        mount_proc = subprocess.Popen(['target/debug/cowblock', 'input.bin', 'cow', '--block-size', '64'])
        time.sleep(2)
        assert mount_proc.returncode is None

        for _ in range(100):
            if os.path.getsize('cow/input.bin') != len(data):
                raise AssertionError("Invalid file size: %d != %d" % (os.path.getsize('cow/input.bin'), len(data)))

            # Do random write
            with open('cow/input.bin', 'r+b') as fp:
                # Only 10% chance to write over the end of the file
                in_bounds = rand.random() > 0.10
                if in_bounds:
                    pos = rand.randint(0, len(data))
                else:
                    pos = rand.randint(0, len(data) + 200)

                # Random buffer
                buf = make_data(
                    rand.randint(0, 300),
                    rand.randint(0, (1 << 32) - 1),
                )

                print('> write(%d, %d)' % (pos, len(buf)), flush=True)
                if pos + len(buf) > len(data):
                    print('(new size %d)' % (pos + len(buf)), flush=True)

                # Do the write
                if pos > len(data):
                    data = data + bytes([0] * (pos - len(data)))
                data = data[:pos] + buf + data[pos + len(buf):]
                fp.seek(pos, 0)
                res = fp.write(buf)

                if res != len(buf):
                    raise AssertionError("Partial write: %d != %d" % (res, len(buf)))

            # Check extra file
            with open('cow-extra', 'rb') as fp:
                extra = fp.read()
            if extra != data[init_data_size - (init_data_size % 64):]:
                raise AssertionError("Invalid extra file content:\n%s\n    !=\n%s" % (hexstring(extra), hexstring(data[init_data_size - (init_data_size % 100):])))
            del extra

            # Check diff file
            with open('cow-diff', 'rb') as fp:
                diff = fp.read()
            for block in range(init_data_size // 64):
                num, = struct.unpack('>L', diff[block * 4:block * 4 + 4])
                if num != 0:
                    pos = (num - 1) * 64
                    pos += (init_data_size // 64) * 4
                    block_data = diff[pos:pos + 64]
                    if block_data != data[block * 64:block * 64 + 64]:
                        raise AssertionError("Invalid diff block %d:\n%s\n    !=\n%s" % (block, hexstring(block_data), hexstring(data[block * 64:block * 64 + 64])))
            del diff

            # Do random read
            with open('cow/input.bin', 'rb') as fp:
                pos = max(0, pos - rand.randint(0, 300))
                # Only 10% chance to request more than the total length of the file
                in_bounds = rand.random() > 0.10
                if in_bounds:
                    size = rand.randint(pos, len(data) - 1) - pos
                else:
                    size = rand.randint(pos, len(data) + 200) - pos

                print('> read(%d, %d)' % (pos, size), flush=True)

                # Do the read
                fp.seek(pos, 0)
                buf = fp.read(size)

                if len(buf) != size:
                    print('(read %d)' % len(buf), flush=True)

                # Check it
                if buf != data[pos:pos + size]:
                    raise AssertionError("Invalid read:\n%s\n    !=\n%s" % (hexstring(buf), hexstring(data[pos:pos + size])))
    finally:
        mount_proc.terminate()
        mount_proc.wait()
        subprocess.call(['fusermount', '-u', 'cow'])

        for filename in ('input.bin', 'cow-diff', 'cow-extra'):
            try:
                os.remove(filename)
            except FileNotFoundError:
                pass

for i in range(20):
    do_test(i)
