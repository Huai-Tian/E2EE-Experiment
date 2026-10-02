#!/usr/bin/env python3
"""跨语言验证：Python 经 ctypes 直接使用 e2ee 库（C ABI 的意义所在）。"""
import ctypes
import sys

lib = ctypes.CDLL("target/release/libe2ee.so")

lib.e2ee_version.restype = ctypes.c_char_p
lib.e2ee_person_create.restype = ctypes.c_void_p
lib.e2ee_person_destroy.argtypes = [ctypes.c_void_p]
lib.e2ee_local_addr.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
lib.e2ee_dial.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint64)]
lib.e2ee_speak.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.e2ee_await_secret.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.e2ee_poll.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]


class Event(ctypes.Structure):
    _fields_ = [
        ("tag", ctypes.c_int), ("id", ctypes.c_uint64),
        ("focused", ctypes.c_int), ("members", ctypes.c_int),
        ("name", ctypes.c_char * 64), ("text", ctypes.c_char * 512),
    ]


print(lib.e2ee_version().decode())
alice = lib.e2ee_person_create(b"Alice", b"127.0.0.1:0")
bob = lib.e2ee_person_create(b"Bob", b"127.0.0.1:0")
assert alice and bob

buf = ctypes.create_string_buffer(64)
lib.e2ee_local_addr(alice, buf, 64)
addr = buf.value.decode()
print(f"Alice 听在 {addr}")

# 暗号 + 拨通
lib.e2ee_await_secret(alice, b"py-ctypes")
lib.e2ee_dial(bob, addr.encode(), b"py-ctypes", None)

# 轮询到 HEARD
ev = Event()
for _ in range(200):
    r = lib.e2ee_poll(alice, 50, ctypes.byref(ev))
    if r == 1 and ev.tag == 4:  # HEARD
        break
    if r == 1 and ev.tag == 2:  # MET
        lib.e2ee_speak(bob, b"python ye neng shuo")
else:
    sys.exit("没等到话")

assert ev.name == b"Bob" and ev.text == b"python ye neng shuo", (ev.name, ev.text)
print(f"heard: [{ev.name.decode()}] {ev.text.decode()}")

lib.e2ee_person_destroy(alice)
lib.e2ee_person_destroy(bob)
print("ctypes OK")
