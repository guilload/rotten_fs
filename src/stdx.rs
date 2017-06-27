use std::os::unix::io::RawFd;


#[derive(Debug, Clone, PartialEq)]
pub enum StdX {
    Pipe(RawFd),
    Redirect(String),
    StdErr,
    StdIn,
    StdOut,
}
