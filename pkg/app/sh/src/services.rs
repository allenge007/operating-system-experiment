use lib::*;
use alloc::vec;

const CAT_BUFFER_SIZE: usize = 512;

pub fn cat_file(path: &str) {
    match open(path) {
        Ok(fd) => {
            let mut buffer = vec![0u8; CAT_BUFFER_SIZE];
            loop {
                let bytes_read_isize = read(fd, &mut buffer); // read returns isize

                if bytes_read_isize < 0 {
                    // Error case
                    errln!("cat: read error (code: {})", bytes_read_isize);
                    break;
                } else if bytes_read_isize == 0 {
                    // EOF
                    break;
                } else {
                    // Successfully read bytes_read_isize bytes
                    let bytes_read = bytes_read_isize as usize;
                    // Attempt to print as UTF-8, fallback for invalid sequences
                    match core::str::from_utf8(&buffer[..bytes_read]) {
                        Ok(s) => print!("{}", s), // Use the imported print
                        Err(_) => {
                            // Fallback: print byte values or a placeholder
                            for byte_idx in 0..bytes_read {
                                let ch = buffer[byte_idx] as char;
                                if ch.is_ascii_graphic() || ch == ' ' || ch == '\n' || ch == '\t' {
                                    print!("{}", ch);
                                } else {
                                    print!("."); // Placeholder for non-printable ASCII or non-ASCII
                                }
                            }
                        }
                    }
                }
            }
            if let Err(e) = close(fd) { // Assuming close returns Result<(), &'static str>
                errln!("cat: close error: {:?}", e);
            }
        }
        Err(e) => {
            errln!("cat: open error for '{}': {:?}", path, e);
        }
    }
}

pub fn exec(name: &str) {
    // let start = sys_time();

    let pid = sys_spawn(name.to_ascii_lowercase().as_str());

    if pid == 0 {
        errln!("failed to spawn process: {}", name);
        return;
    }

    let ret = sys_wait_pid(pid);
    // let time = sys_time() - start;

    println!(
        "process exited with code {}",
        ret,
        // time.num_seconds()
    );
}

pub fn kill(pid: u16) {
    sys_kill(pid);
}
