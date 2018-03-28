use std::error::Error;
use winapi;
use kernel32;
use isatty;
use libc;

pub struct Terminal {
    handle: winapi::HANDLE;
    mode: winapi::DWORD;
}

impl Terminal {
    pub fn new() -> Result<Self, Box<Error>> {
        let handle = kernel32::GetStdHandle(winapi::winbase::STD_OUTPUT_HANDLE);
        let mode: winapi::DWORD;
        unsafe{ kernel32::GetConsoleMode(handle, &mode); }
        Ok(Terminal{
            handle,
            mode,
        })
    }

    pub fn set(&self) {
        if !isatty(isatty::stream::Stream::Stdout) {
            return
        }
    
        
    }
    
    pub fn reset(&self) {
        if !isatty(isatty::stream::Stream::Stdout) {
            return
        }

    }

}

pub fn read_key() -> Option<char> {
}
