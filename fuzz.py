#!/usr/bin/env python3

import os
import random
import subprocess
import time


# Make test data
def make_data(size, seed):
    rand = random.Random(seed)
    return bytes(rand.randint(0, 255) for _ in range(size))


def do_test(seed):
    print("do_test(seed=%d)" % seed, flush=True)

    # Make data array
    rand = random.Random(seed)
    data = make_data(
        rand.randint(30, 600),
        rand.randint(0, (1 << 32) - 1),
    )

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

                # Do the write
                if pos > len(data):
                    data = data + bytes([0] * (pos - len(data)))
                data = data[:pos] + buf + data[pos + len(buf):]
                fp.seek(pos, 0)
                res = fp.write(buf)

                if res != len(buf):
                    raise AssertionError("Partial write: %d != %d" % (res, len(buf)))

            # Do random read
            with open('cow/input.bin', 'rb') as fp:
                pos = max(0, pos - rand.randint(0, 300))
                # Only 10% chance to request more than the total length of the file
                in_bounds = rand.random() > 0.10
                if in_bounds:
                    size = rand.randint(pos, len(data) - 1) - pos
                else:
                    size = rand.randint(pos, len(data) + 200) - pos

                # Do the read
                fp.seek(pos, 0)
                buf = fp.read(size)

                # Check it
                if buf != data[pos:pos + size]:
                    raise AssertionError("Invalid read:\n%r\n    !=\n%r" % (buf, data[pos:pos + size]))
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
