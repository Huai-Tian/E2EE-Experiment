#!/usr/bin/env python3
"""跨语言验证：Python 经 ctypes 直接使用 e2ee 库（C ABI 的意义所在）。"""
import ctypes
import sys

lib = ctypes.CDLL("target/release/libe2ee.so")

lib.e2ee_version.restype = ctypes.c_char_p
lib.e2ee_person_create.restype = ctypes.c_void_p
lib.e2ee_person_create_hidden.restype = ctypes.c_void_p
lib.e2ee_person_destroy.argtypes = [ctypes.c_void_p]
lib.e2ee_local_addr.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
lib.e2ee_dial.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint64)]
lib.e2ee_speak.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.e2ee_await_secret.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.e2ee_poll.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
# 4.1.0 P22 同意门槛
lib.e2ee_file_begin.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint64]
lib.e2ee_file_chunk.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
lib.e2ee_file_end.argtypes = [ctypes.c_void_p]
lib.e2ee_file_yes.argtypes = [ctypes.c_void_p]
lib.e2ee_file_no.argtypes = [ctypes.c_void_p]
lib.e2ee_set_receive.argtypes = [ctypes.c_void_p, ctypes.c_int]


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


def wait_for_tag(person, tag, who):
    for _ in range(200):
        r = lib.e2ee_poll(person, 50, ctypes.byref(ev))
        if r == 1 and ev.tag == tag:
            return
        if r < 0:
            sys.exit(f"{who} 事件流出错")
    sys.exit(f"{who} 没等到事件 {tag}")


# 文件同意门槛（4.1.0 P22）：先问后答全程 + 回执照到
payload = b"0123456789"
assert lib.e2ee_file_begin(bob, b"py-file.dat", len(payload)) == 0
assert lib.e2ee_file_chunk(bob, payload, len(payload)) != 0, "应允前喂块必须响亮失败"
wait_for_tag(alice, 14, "Alice")  # FILE_REQUESTED
assert ev.name == b"Bob" and ev.text.startswith(b"py-file.dat"), (ev.name, ev.text)
assert lib.e2ee_file_yes(alice) == 0
wait_for_tag(bob, 15, "Bob")  # FILE_ACCEPTED
assert lib.e2ee_file_chunk(bob, payload, len(payload)) == 0
assert lib.e2ee_file_end(bob) == 0
wait_for_tag(alice, 12, "Alice")  # FILE_COMPLETE
wait_for_tag(bob, 17, "Bob")  # FILE_DELIVERED（收讫回执）
assert lib.e2ee_file_yes(alice) != 0, "no 后不可 yes——问题已死"
# 态度：设 never 再递 → 政策拒（members = 3）
assert lib.e2ee_set_receive(alice, 2) == 0
assert lib.e2ee_file_begin(bob, b"ju-jue.txt", 4) == 0
wait_for_tag(bob, 16, "Bob")  # FILE_DECLINED
assert ev.members == 3, f"政策拒 members 应为 3：{ev.members}"
assert lib.e2ee_set_receive(alice, 0) == 0
print("file consent gate OK")

# 已废弃的别名（3.0.0 起等同 create）：符号保留验 ABI 冻结
hidden = lib.e2ee_person_create_hidden(b"Hidden", b"127.0.0.1:0")
assert hidden
lib.e2ee_person_destroy(hidden)
print("hidden OK")

lib.e2ee_person_destroy(alice)
lib.e2ee_person_destroy(bob)
print("ctypes OK")
