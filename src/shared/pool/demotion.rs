use std::io;

#[cfg(target_os = "macos")]
pub fn demote() -> io::Result<()> {
    const CLASS: libc::qos_class_t = libc::qos_class_t::QOS_CLASS_UTILITY;

    let rc = unsafe { libc::pthread_set_qos_class_self_np(CLASS, 0) };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[rustfmt::skip]
pub fn demote() -> io::Result<()> {
    use std::mem;

    const SHED_POLICY: libc::c_int = libc::SCHED_BATCH;
    const PRIO: libc::c_int = 10;

    let param = mem::MaybeUninit::zeroed();
    let rc = unsafe {
        libc::pthread_setschedparam(libc::pthread_self(), SHED_POLICY, param.as_ptr())
    };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }

    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, PRIO) } == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn demote() -> io::Result<()> {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY, THREAD_PRIORITY_BELOW_NORMAL,
    };

    const PRIORITY: THREAD_PRIORITY = THREAD_PRIORITY_BELOW_NORMAL;

    if unsafe { SetThreadPriority(GetCurrentThread(), PRIORITY) } == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn demote() -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
