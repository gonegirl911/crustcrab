#[cfg(target_os = "macos")]
pub fn demote() {
    unsafe {
        _ = libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
}

#[cfg(target_os = "linux")]
pub fn demote() {
    const NICE: libc::c_int = 10;

    let param = libc::sched_param { sched_priority: 0 };
    unsafe {
        _ = libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_BATCH, &param);
        _ = libc::setpriority(libc::PRIO_PROCESS, 0, NICE);
    }
}

#[cfg(target_os = "windows")]
pub fn demote() {
    #[expect(non_snake_case)]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut core::ffi::c_void;
        fn SetThreadPriority(thread: *mut core::ffi::c_void, priority: i32) -> i32;
    }

    const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;

    unsafe {
        _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn demote() {}
