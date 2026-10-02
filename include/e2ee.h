/*
 * E2EE-Experiment —— C ABI
 * 一个像人一样的端到端加密对话内核：能听，能说，转身即忘。
 *
 * 与 src/ffi.rs 同步维护（ABI 冻结：只加不改）。
 * 线程模型：句柄可跨线程传递；句柄的生灭（create/destroy）请在同一线程
 * 串行管理，别的线程还在 poll 时不要 destroy。
 * 返回值：0 = 成功，-1 = 失败（人话在 e2ee_last_error，中文）。
 *
 * 最小示例：
 *     e2ee_person* me = e2ee_person_create("Alice", "127.0.0.1:0");
 *     char addr[64]; e2ee_local_addr(me, addr, sizeof addr);
 *     e2ee_await_secret(other, "lan-tong-qi");
 *     e2ee_dial(me, addr, "lan-tong-qi", NULL);
 *     e2ee_speak(me, "ni hao");
 *     e2ee_event ev;
 *     while (e2ee_poll(other, 100, &ev) == 1) { if (ev.tag == E2EE_EVENT_HEARD) ... }
 *     e2ee_bye(me); e2ee_person_destroy(me);
 */
#ifndef E2EE_H
#define E2EE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

typedef struct e2ee_person e2ee_person; /* 不透明句柄 */

/* 事件标签（冻结）：tag 与字段的对应见 e2ee_event */
enum {
    E2EE_EVENT_UNKNOWN = 0,
    E2EE_EVENT_KNOCKED = 1,        /* name = 来客地址 */
    E2EE_EVENT_MET = 2,            /* id, name, focused */
    E2EE_EVENT_MEET_FAILED = 3,    /* text = 失败原因 */
    E2EE_EVENT_HEARD = 4,          /* id, name, text */
    E2EE_EVENT_LEFT = 5,           /* id, name, text = 结束原因 */
    E2EE_EVENT_FOCUS_RETURNED = 6, /* id = 注意力回到哪场（0 = 没了） */
    E2EE_EVENT_SECRET_CHAINED = 7, /* name = 与谁续上了暗号链 */
    E2EE_EVENT_CIRCLE_FORMED = 8,  /* members = 圈内人数 */
    E2EE_EVENT_CIRCLE_MUMBLE = 9,  /* name = 谁说了句解不开的话 */
};

/* 事件（布局冻结）：超容量的字符串被截断（name 63 字节、text 511 字节） */
typedef struct e2ee_event {
    int tag;                /* 上面的 E2EE_EVENT_* */
    uint64_t id;            /* 对话编号 */
    int focused;            /* MET：是否成为注意力所在 */
    int members;            /* CIRCLE_FORMED：圈内人数 */
    char name[64];          /* 对方名字 / 来客地址 / 链对象 */
    char text[512];         /* 听到的话 / 失败或结束原因 */
} e2ee_event;

const char*    e2ee_version(void);

/* 生灭 */
e2ee_person*   e2ee_person_create(const char* name, const char* listen_addr); /* NULL 失败 */
void           e2ee_person_destroy(e2ee_person* p);                           /* 体面道别后熄灭 */

/* 查询 */
int            e2ee_local_addr(e2ee_person* p, char* buf, int buflen);
int            e2ee_last_error(char* buf, int buflen); /* 线程局部，下次调用前有效 */

/* 动词 */
int            e2ee_await_secret(e2ee_person* p, const char* secret);
int            e2ee_dial(e2ee_person* p, const char* addr, const char* secret /*可空*/,
                         uint64_t* out_convo_id /*可空*/);
int            e2ee_speak(e2ee_person* p, const char* text);
int            e2ee_bye(e2ee_person* p);
int            e2ee_leave(e2ee_person* p);

/* 事件：返回 1 = 拿到，0 = 超时，-1 = 出错；不关心的事件会被丢弃 */
int            e2ee_poll(e2ee_person* p, int timeout_ms /*<0 死等，0 非阻塞*/,
                         e2ee_event* out_event);

#ifdef __cplusplus
}
#endif

#endif /* E2EE_H */
