/* HYWOS 混合内核核心（C 实现）
 *
 * 负责：
 *   - 子系统注册表与状态机（Kernel/Video/Input/Storage/Network/Audio/UI/Ring3/Updates）
 *   - 内核能力位与运行计数器
 *   - 驱动总线装配（调用 C++ 驱动框架）
 *   - 自检
 *
 * Rust 只负责硬件探测与界面；探测结果通过 hyw_subsys_set 上报本内核。
 */
#include "hyw_kernel.h"

typedef struct {
    char name[32];
    char detail[80];
    int state;
    int used;
} HywSubsys;

static HywSubsys gSubs[HYW_MAX_SUBSYS];
static unsigned int gSubCount = 0;
static unsigned long long gTicks = 0;
static int gInited = 0;

static void copy_str(char *dst, unsigned int cap, const char *src) {
    unsigned int i = 0;
    if (dst == 0 || cap == 0) {
        return;
    }
    if (src == 0) {
        dst[0] = 0;
        return;
    }
    while (src[i] != 0 && i + 1 < cap) {
        dst[i] = src[i];
        i++;
    }
    dst[i] = 0;
}

static int register_one(const char *name, const char *detail) {
    HywSubsys *s;
    if (gSubCount >= HYW_MAX_SUBSYS) {
        return -1;
    }
    s = &gSubs[gSubCount];
    copy_str(s->name, sizeof(s->name), name);
    copy_str(s->detail, sizeof(s->detail), detail);
    s->state = HYW_DOWN;
    s->used = 1;
    return (int)gSubCount++;
}

HYW_KERNEL_API void hyw_kernel_init(void) {
    unsigned int i;
    int k;
    if (gInited) {
        return;
    }
    gInited = 1;

    /* 固定顺序：Rust 侧按 HYW_SUB_* 上报。 */
    k = register_one("Kernel Core", "C/C++ 混合内核核心");
    k = register_one("Video", "图形子系统");
    k = register_one("Input", "输入子系统");
    k = register_one("Storage", "存储子系统");
    k = register_one("Network", "网络子系统");
    k = register_one("Audio", "音频子系统");
    k = register_one("UI", "Rust 合成器与桌面");
    k = register_one("Ring3", "用户态与双页表");
    k = register_one("Updates", "系统更新");
    (void)k;

    hyw_drivers_bootstrap();

    if (gSubCount > HYW_SUB_KERNEL) {
        gSubs[HYW_SUB_KERNEL].state = HYW_READY;
        copy_str(gSubs[HYW_SUB_KERNEL].detail,
                 sizeof(gSubs[HYW_SUB_KERNEL].detail),
                 "C/C++ 混合内核 · C++ 驱动总线");
    }

    /* 预绑定驱动，形成 builtin + module 的混合驱动架构。 */
    hyw_driver_bind_all();

    /* 保持未使用槽位归零。 */
    for (i = gSubCount; i < HYW_MAX_SUBSYS; i++) {
        gSubs[i].used = 0;
        gSubs[i].state = HYW_DOWN;
        gSubs[i].name[0] = 0;
        gSubs[i].detail[0] = 0;
    }
}

HYW_KERNEL_API const char *hyw_kernel_kind(void) {
    return "hybrid";
}

HYW_KERNEL_API const char *hyw_kernel_arch(void) {
    return "x86_64 hybrid (UEFI+BIOS)";
}

HYW_KERNEL_API const char *hyw_kernel_version(void) {
    return "1.1.6";
}

HYW_KERNEL_API unsigned int hyw_kernel_caps(void) {
    unsigned int c = 0;
    c |= 1u << 0; /* 混合内核 */
    c |= 1u << 1; /* 内核态核心服务 */
    c |= 1u << 2; /* 模块化服务层 */
    c |= 1u << 3; /* 内置驱动 */
    c |= 1u << 4; /* 模块驱动 */
    c |= 1u << 5; /* UEFI 引导 */
    c |= 1u << 6; /* BIOS 引导 */
    return c;
}

HYW_KERNEL_API unsigned int hyw_subsys_count(void) {
    return gSubCount;
}

HYW_KERNEL_API const char *hyw_subsys_name(unsigned int index) {
    if (index >= gSubCount) {
        return "";
    }
    return gSubs[index].name;
}

HYW_KERNEL_API const char *hyw_subsys_detail(unsigned int index) {
    if (index >= gSubCount) {
        return "";
    }
    return gSubs[index].detail;
}

HYW_KERNEL_API int hyw_subsys_state(unsigned int index) {
    if (index >= gSubCount) {
        return HYW_DOWN;
    }
    return gSubs[index].state;
}

HYW_KERNEL_API unsigned int hyw_subsys_ready(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < gSubCount; i++) {
        if (gSubs[i].state == HYW_READY) {
            n++;
        }
    }
    return n;
}

HYW_KERNEL_API unsigned int hyw_subsys_active(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < gSubCount; i++) {
        if (gSubs[i].state != HYW_DOWN) {
            n++;
        }
    }
    return n;
}

HYW_KERNEL_API void hyw_subsys_set(unsigned int index, int state, const char *detail) {
    if (index >= gSubCount) {
        return;
    }
    if (state < HYW_DOWN) {
        state = HYW_DOWN;
    }
    if (state > HYW_READY) {
        state = HYW_READY;
    }
    gSubs[index].state = state;
    if (detail != 0) {
        copy_str(gSubs[index].detail, sizeof(gSubs[index].detail), detail);
    }
}

HYW_KERNEL_API int hyw_subsys_register(const char *name, const char *detail) {
    return register_one(name, detail);
}

HYW_KERNEL_API unsigned long long hyw_kernel_ticks(void) {
    return gTicks;
}

HYW_KERNEL_API void hyw_kernel_tick(void) {
    gTicks++;
}

HYW_KERNEL_API int hyw_selftest(void) {
    unsigned int i;
    unsigned int ready;
    if (gSubCount < HYW_SUB_TOTAL_FIXED) {
        return 1;
    }
    if (hyw_driver_count() == 0) {
        return 2;
    }
    ready = hyw_subsys_ready();
    if (ready > gSubCount) {
        return 3;
    }
    for (i = 0; i < gSubCount; i++) {
        if (gSubs[i].state != HYW_DOWN &&
            gSubs[i].state != HYW_DEGRADED &&
            gSubs[i].state != HYW_READY) {
            return 4;
        }
    }
    if (hyw_driver_bound() > hyw_driver_count()) {
        return 5;
    }
    return 0;
}