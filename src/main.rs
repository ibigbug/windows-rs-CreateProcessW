windows::include_bindings!();

use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::RawHandle;
use std::ptr::null_mut;
use std::thread;

use Windows::Win32::Foundation::*;
use Windows::Win32::System::Console::*;
use Windows::Win32::System::Pipes::*;
use Windows::Win32::System::Threading::*;
use Windows::Win32::System::WindowsProgramming::*;

extern crate windows as w;
use w::HRESULT;

static PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;

fn main() -> windows::Result<()> {
    // create pesuedo con handles
    let mut h_pc: HPCON;
    let mut stdin = INVALID_HANDLE_VALUE;
    let mut stdout = INVALID_HANDLE_VALUE;

    let mut h_pipe_pty_in = INVALID_HANDLE_VALUE;
    let mut h_pipe_pty_out = INVALID_HANDLE_VALUE;

    unsafe {
        if !CreatePipe(&mut h_pipe_pty_in, &mut stdin, null_mut(), 0).as_bool() {
            panic!("cannot create pipe");
        }
        if !CreatePipe(&mut stdout, &mut h_pipe_pty_out, null_mut(), 0).as_bool() {
            panic!("cannot create pipe");
        }
    }

    let mut console_size = COORD::default();
    unsafe {
        console_size.X = 140;
        console_size.Y = 80;
        h_pc = CreatePseudoConsole(console_size, h_pipe_pty_in, h_pipe_pty_out, 0)
            .expect("Cant create PseudoConsole");
    }

    unsafe {
        let mut file_in = File::from_raw_handle((&mut stdin as *mut HANDLE).cast::<_>());
        let mut file_out = File::from_raw_handle((&mut stdout as *mut HANDLE).cast::<_>());

        thread::spawn(move || loop {
            let mut buffer = [0u8; 1024];
            match file_out.read(&mut buffer) {
                Ok(n) if n > 0 => {
                    io::stdout()
                        .write_all(&buffer[..n])
                        .expect("write stdout fail");
                }
                _ => {
                    break;
                }
            }
        });
    }

    let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

    let mut lp_size: usize = 0;
    let mut success: BOOL;
    unsafe {
        success = InitializeProcThreadAttributeList(
            LPPROC_THREAD_ATTRIBUTE_LIST::NULL,
            1,
            0,
            &mut lp_size,
        );
        // Note: This initial call will return an error by design. This is expected behavior.
        if success.as_bool() || lp_size == 0 {
            let err = HRESULT::from_thread();
            panic!(
                "Can't calculate the number of bytes for the attribute list, {}",
                err.message()
            );
        }
    }

    let mut lp_attribute_list: Box<[u8]> = vec![0; lp_size].into_boxed_slice();
    si.lpAttributeList = LPPROC_THREAD_ATTRIBUTE_LIST(lp_attribute_list.as_mut_ptr().cast::<_>());

    success = unsafe { InitializeProcThreadAttributeList(si.lpAttributeList, 1, 0, &mut lp_size) };
    if !success.as_bool() {
        let err = HRESULT::from_thread();
        panic!("Can't setup attribute list, {}", err.message());
    }

    success = unsafe {
        UpdateProcThreadAttribute(
            si.lpAttributeList,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            (&mut h_pc as *mut HPCON).cast::<_>(),
            std::mem::size_of::<HPCON>(),
            null_mut(),
            null_mut(),
        )
    };

    if !success.as_bool() {
        let err = HRESULT::from_thread();
        panic!("Can't setup process attribute, {}", err.message());
    }

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let cmd = "ping.exe";
    unsafe {
        let success = CreateProcessW(
            PWSTR::NULL,
            cmd,
            null_mut(),
            null_mut(),
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            null_mut(),
            PWSTR::NULL,
            &mut si.StartupInfo,
            &mut pi,
        );
        if !success.as_bool() {
            let err = HRESULT::from_thread();
            panic!("Cant create process: {:?}", err.message());
        }
        WaitForSingleObject(pi.hProcess, INFINITE);

        let mut exit_code: u32 = 0;

        GetExitCodeProcess(pi.hProcess, &mut exit_code);
        println!("exitcode: {}", exit_code);
    }
    Ok(())
}
