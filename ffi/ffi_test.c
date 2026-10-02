/* C ABI 契约测试：真实起两个人、真实握手、真实密谈。
 * 编译运行见 README_AGENT.md 的 verify 段。
 * 通过标准：全部 CHECK 打 ok，最后一行 FFI OK。
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include "e2ee.h"

static int failures = 0;
#define CHECK(cond, msg) \
    do { if (cond) { printf("ok: %s\n", msg); } else { printf("FAIL: %s\n", msg); failures++; } } while (0)

static void show_err(const char* what) {
    char buf[256];
    e2ee_last_error(buf, sizeof buf);
    printf("  (%s) last_error: %s\n", what, buf);
}

/* 等一件指定的事（其余事件丢弃），最多约 10 秒 */
static int wait_for(e2ee_person* p, int tag, e2ee_event* out) {
    for (int i = 0; i < 200; i++) {
        int r = e2ee_poll(p, 50, out);
        if (r == 1 && out->tag == tag) return 1;
        if (r < 0) break;
    }
    return 0;
}

int main(void) {
    printf("%s\n", e2ee_version());

    /* 出生：端口 0，真地址问库要 */
    e2ee_person* a = e2ee_person_create("Alice", "127.0.0.1:0");
    CHECK(a != NULL, "create A");
    e2ee_person* b = e2ee_person_create("Bob", "127.0.0.1:0");
    CHECK(b != NULL, "create B");
    if (!a || !b) { show_err("create"); return 1; }

    char addr[64];
    CHECK(e2ee_local_addr(a, addr, sizeof addr) == 0 && addr[0] != '\0', "local_addr A");
    printf("  A 听在 %s\n", addr);

    /* 暗号：A 备好切口，B 带同一句进来（PAKE） */
    CHECK(e2ee_await_secret(a, "lan-tong-qi-hao") == 0, "await_secret A");

    uint64_t id = 0;
    int r = e2ee_dial(b, addr, "lan-tong-qi-hao", &id);
    CHECK(r == 0 && id > 0, "dial B->A with secret");
    if (r != 0) show_err("dial");

    e2ee_event ev;
    CHECK(wait_for(b, E2EE_EVENT_MET, &ev) && strcmp(ev.name, "Alice") == 0,
          "B met Alice");
    CHECK(wait_for(a, E2EE_EVENT_MET, &ev) && strcmp(ev.name, "Bob") == 0,
          "A met Bob");

    /* 双向密谈 */
    CHECK(e2ee_speak(b, "ni hao ma?") == 0, "speak B");
    CHECK(wait_for(a, E2EE_EVENT_HEARD, &ev) && strcmp(ev.text, "ni hao ma?") == 0,
          "A heard B");
    CHECK(e2ee_speak(a, "ting de dao") == 0, "speak A");
    CHECK(wait_for(b, E2EE_EVENT_HEARD, &ev) && strcmp(ev.text, "ting de dao") == 0,
          "B heard A");

    /* 道别 + 暗号链：A 应看到 SECRET_CHAINED 与 LEFT */
    CHECK(e2ee_bye(b) == 0, "bye B");
    CHECK(wait_for(a, E2EE_EVENT_SECRET_CHAINED, &ev) && strcmp(ev.name, "Bob") == 0,
          "A secret chained");
    CHECK(wait_for(a, E2EE_EVENT_LEFT, &ev), "A saw Bob left");
    CHECK(wait_for(b, E2EE_EVENT_LEFT, &ev), "B saw line closed");

    /* 错误路径：A 竖新切口，B 用错暗号必须失败（响亮，不无声放行） */
    CHECK(e2ee_await_secret(a, "di-er-ba") == 0, "await_secret A (2nd)");
    CHECK(e2ee_dial(b, addr, "cuo-de", &id) != 0, "wrong secret must fail");
    show_err("wrong secret");

    /* 没在跟人说话时开口：报错而非崩溃 */
    CHECK(e2ee_speak(b, "dui kong qi shuo") != 0, "speak without focus errors");
    show_err("speak no focus");

    /* 隐身出生：探不到，但精确交往照常——这里验精确拨通 */
    e2ee_person* h = e2ee_person_create_hidden("Hidden", "127.0.0.1:0");
    CHECK(h != NULL, "create hidden H");
    if (h) {
        char haddr[64];
        CHECK(e2ee_local_addr(h, haddr, sizeof haddr) == 0 && haddr[0] != '\0',
              "local_addr hidden");
        CHECK(e2ee_dial(h, addr, NULL, &id) == 0 && id > 0, "hidden dials A by exact address");
        CHECK(wait_for(a, E2EE_EVENT_MET, &ev) && strcmp(ev.name, "Hidden") == 0,
              "A met hidden H");
        CHECK(e2ee_speak(h, "yin shen zhe zhao yang neng shuo") == 0, "speak hidden");
        CHECK(wait_for(a, E2EE_EVENT_HEARD, &ev)
                  && strcmp(ev.text, "yin shen zhe zhao yang neng shuo") == 0,
              "A heard hidden H");
        e2ee_person_destroy(h);
        CHECK(1, "destroy hidden");
    }

    e2ee_person_destroy(a);
    e2ee_person_destroy(b);
    CHECK(1, "destroy both");

    if (failures == 0) {
        printf("FFI OK\n");
        return 0;
    }
    printf("FFI FAILED: %d\n", failures);
    return 1;
}
