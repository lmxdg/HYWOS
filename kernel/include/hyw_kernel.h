/* HYWOS 混合内核公共 C API
 * 内核核心与驱动总线由 C/C++ 实现，界面与桌面由 Rust 保持。
 * 所有导出函数使用 Microsoft x64 ABI (ms_abi)，与 Rust extern "efiapi" 对齐。
 */
#ifndef HYW_KERNEL_H
#define HYW_KERNEL_H

#define HYW_ABI __attribute__((ms_abi))
#define HYW_KERNEL_API __attribute__((visibility("hidden"))) HYW_ABI

#ifdef __cplusplus
extern "C" {
#endif

enum {
    HYW_DOWN = 0,
    HYW_DEGRADED = 1,
    HYW_READY = 2
};

enum {
    HYW_BUS_NONE = 0,
    HYW_BUS_PCI = 1,
    HYW_BUS_USB = 2,
    HYW_BUS_VIRTIO = 3,
    HYW_BUS_PS2 = 4,
    HYW_BUS_PLATFORM = 5
};

enum {
    HYW_DRIVER_BUILTIN = 0,
    HYW_DRIVER_MODULE = 1
};

#define HYW_MAX_SUBSYS 12
#define HYW_MAX_DRIVERS 40

/* 固定子系统索引，Rust 侧按同一顺序上报状态。 */
enum {
    HYW_SUB_KERNEL = 0,
    HYW_SUB_VIDEO = 1,
    HYW_SUB_INPUT = 2,
    HYW_SUB_STORAGE = 3,
    HYW_SUB_NETWORK = 4,
    HYW_SUB_AUDIO = 5,
    HYW_SUB_UI = 6,
    HYW_SUB_RING3 = 7,
    HYW_SUB_UPDATES = 8,
    HYW_SUB_TOTAL_FIXED = 9
};

HYW_KERNEL_API void hyw_kernel_init(void);
HYW_KERNEL_API const char *hyw_kernel_kind(void);
HYW_KERNEL_API const char *hyw_kernel_arch(void);
HYW_KERNEL_API const char *hyw_kernel_version(void);
HYW_KERNEL_API unsigned int hyw_kernel_caps(void);

HYW_KERNEL_API unsigned int hyw_subsys_count(void);
HYW_KERNEL_API const char *hyw_subsys_name(unsigned int index);
HYW_KERNEL_API const char *hyw_subsys_detail(unsigned int index);
HYW_KERNEL_API int hyw_subsys_state(unsigned int index);
HYW_KERNEL_API unsigned int hyw_subsys_ready(void);
HYW_KERNEL_API unsigned int hyw_subsys_active(void);
HYW_KERNEL_API void hyw_subsys_set(unsigned int index, int state, const char *detail);
HYW_KERNEL_API int hyw_subsys_register(const char *name, const char *detail);

HYW_KERNEL_API unsigned long long hyw_kernel_ticks(void);
HYW_KERNEL_API void hyw_kernel_tick(void);

/* 驱动总线（由 C++ 实现）。 */
HYW_KERNEL_API void hyw_drivers_bootstrap(void);
HYW_KERNEL_API unsigned int hyw_driver_count(void);
HYW_KERNEL_API unsigned int hyw_driver_bound(void);
HYW_KERNEL_API unsigned int hyw_driver_builtin(void);
HYW_KERNEL_API unsigned int hyw_driver_module(void);
HYW_KERNEL_API const char *hyw_driver_name(unsigned int index);
HYW_KERNEL_API int hyw_driver_bus(unsigned int index);
HYW_KERNEL_API int hyw_driver_kind(unsigned int index);
HYW_KERNEL_API int hyw_driver_is_bound(unsigned int index);
HYW_KERNEL_API unsigned int hyw_driver_bind_all(void);
HYW_KERNEL_API int hyw_driver_register(const char *name, int bus, int kind,
                                       unsigned int vendor, unsigned int device,
                                       int (*probe)(void *ctx), void *ctx);

HYW_KERNEL_API int hyw_selftest(void);

#ifdef __cplusplus
}
#endif

#endif /* HYW_KERNEL_H */