/* HYWOS 混合驱动总线（C++ 实现）
 *
 * 混合驱动架构：
 *   - builtin：随内核核心一起驻留的内置驱动（固件帧缓冲、PS/2、VFS、AC'97…）
 *   - module ：以模块描述注册、按总线匹配后绑定的驱动（virtio-gpu、SVGA、
 *              e1000、RTL8139、virtio-net、USB HID、UEFI SNP…）
 *
 * 总线负责枚举、匹配、probe 与生命周期记录，Rust 负责具体硬件寄存器访问。
 */
#include "hyw_kernel.h"

namespace hyw {

struct Driver {
    char name[40];
    int bus;
    int kind;
    unsigned int vendor;
    unsigned int device;
    int (*probe)(void *ctx);
    void *ctx;
    int present;
    int bound;
};

static Driver gDrivers[HYW_MAX_DRIVERS];
static unsigned int gCount = 0;

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

static int default_probe(void *ctx) {
    (void)ctx;
    return 0;
}

static void add(const char *name, int bus, int kind, unsigned int vendor,
                unsigned int device, int (*probe)(void *ctx)) {
    Driver *d;
    if (gCount >= HYW_MAX_DRIVERS) {
        return;
    }
    d = &gDrivers[gCount++];
    copy_str(d->name, sizeof(d->name), name);
    d->bus = bus;
    d->kind = kind;
    d->vendor = vendor;
    d->device = device;
    d->probe = (probe != 0) ? probe : default_probe;
    d->ctx = 0;
    d->present = 1;
    d->bound = 0;
}

} /* namespace hyw */

extern "C" {

HYW_ABI void hyw_drivers_bootstrap(void) {
    using namespace hyw;
    if (gCount != 0) {
        return;
    }

    /* ---- 内置驱动（builtin，随内核驻留） ---- */
    add("gop-framebuffer", HYW_BUS_PLATFORM, HYW_DRIVER_BUILTIN, 0x0000, 0x0000, 0);
    add("ps2-keyboard",    HYW_BUS_PS2,      HYW_DRIVER_BUILTIN, 0x0000, 0x0000, 0);
    add("ps2-mouse",       HYW_BUS_PS2,      HYW_DRIVER_BUILTIN, 0x0000, 0x0000, 0);
    add("fat-vfs",         HYW_BUS_PLATFORM, HYW_DRIVER_BUILTIN, 0x0000, 0x0000, 0);
    add("uefi-block",      HYW_BUS_PLATFORM, HYW_DRIVER_BUILTIN, 0x0000, 0x0000, 0);
    add("ac97-audio",      HYW_BUS_PCI,      HYW_DRIVER_BUILTIN, 0x8086, 0x2415, 0);

    /* ---- 模块驱动（module，按总线匹配后绑定） ---- */
    add("virtio-gpu",      HYW_BUS_VIRTIO,   HYW_DRIVER_MODULE, 0x1AF4, 0x1050, 0);
    add("vmware-svga",     HYW_BUS_PCI,      HYW_DRIVER_MODULE, 0x15AD, 0x0405, 0);
    add("bochs-vbe",       HYW_BUS_PLATFORM, HYW_DRIVER_MODULE, 0x1234, 0x1111, 0);
    add("e1000",           HYW_BUS_PCI,      HYW_DRIVER_MODULE, 0x8086, 0x100E, 0);
    add("e1000e",          HYW_BUS_PCI,      HYW_DRIVER_MODULE, 0x8086, 0x10D3, 0);
    add("rtl8139",         HYW_BUS_PCI,      HYW_DRIVER_MODULE, 0x10EC, 0x8139, 0);
    add("virtio-net",      HYW_BUS_VIRTIO,   HYW_DRIVER_MODULE, 0x1AF4, 0x1000, 0);
    add("usb-hid",         HYW_BUS_USB,      HYW_DRIVER_MODULE, 0x0000, 0x0000, 0);
    add("uefi-snp",        HYW_BUS_PLATFORM, HYW_DRIVER_MODULE, 0x0000, 0x0000, 0);
    add("uefi-absolute-ptr", HYW_BUS_PLATFORM, HYW_DRIVER_MODULE, 0x0000, 0x0000, 0);
}

HYW_ABI unsigned int hyw_driver_count(void) {
    return hyw::gCount;
}

HYW_ABI unsigned int hyw_driver_bound(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < hyw::gCount; i++) {
        if (hyw::gDrivers[i].bound != 0) {
            n++;
        }
    }
    return n;
}

HYW_ABI unsigned int hyw_driver_builtin(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < hyw::gCount; i++) {
        if (hyw::gDrivers[i].kind == HYW_DRIVER_BUILTIN) {
            n++;
        }
    }
    return n;
}

HYW_ABI unsigned int hyw_driver_module(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < hyw::gCount; i++) {
        if (hyw::gDrivers[i].kind == HYW_DRIVER_MODULE) {
            n++;
        }
    }
    return n;
}

HYW_ABI const char *hyw_driver_name(unsigned int index) {
    if (index >= hyw::gCount) {
        return "";
    }
    return hyw::gDrivers[index].name;
}

HYW_ABI int hyw_driver_bus(unsigned int index) {
    if (index >= hyw::gCount) {
        return HYW_BUS_NONE;
    }
    return hyw::gDrivers[index].bus;
}

HYW_ABI int hyw_driver_kind(unsigned int index) {
    if (index >= hyw::gCount) {
        return HYW_DRIVER_BUILTIN;
    }
    return hyw::gDrivers[index].kind;
}

HYW_ABI int hyw_driver_is_bound(unsigned int index) {
    if (index >= hyw::gCount) {
        return 0;
    }
    return hyw::gDrivers[index].bound;
}

HYW_ABI unsigned int hyw_driver_bind_all(void) {
    unsigned int i;
    unsigned int n = 0;
    for (i = 0; i < hyw::gCount; i++) {
        hyw::Driver *d = &hyw::gDrivers[i];
        if (!d->present) {
            continue;
        }
        if (d->probe(d->ctx) == 0) {
            d->bound = 1;
            n++;
        } else {
            d->bound = 0;
        }
    }
    return n;
}

HYW_ABI int hyw_driver_register(const char *name, int bus, int kind,
                                unsigned int vendor, unsigned int device,
                                int (*probe)(void *ctx), void *ctx) {
    unsigned int before;
    using namespace hyw;
    before = gCount;
    add(name, bus, kind, vendor, device, probe);
    if (gCount == before) {
        return -1;
    }
    gDrivers[before].ctx = ctx;
    return (int)before;
}

} /* extern "C" */