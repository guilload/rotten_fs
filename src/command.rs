extern crate libc;

extern crate nix;
extern crate nom;

use std::ffi::CString;
use std::ffi;
use std::fs::File;
use std::io;
use std::os::unix::io::{IntoRawFd, RawFd};

use self::nix::unistd::{close, dup2, execvp, fork, ForkResult, setpgid};
use self::nom::*;

use signal::Signal;
use stdx::StdX;


// nom parsers
named!(parse_redirect_to<&str, StdX>,
    do_parse!(
        tag!(">") >>
        path: ws!(is_not!(" |")) >>

        (StdX::Redirect(path.to_string()))
    )
);

named!(parse_redirect_from<&str, StdX>,
    do_parse!(
        tag!("<") >>
        path: ws!(is_not!(" >|")) >>

        (StdX::Redirect(path.to_string()))
    )
);

named!(pub parse_command<&str, Command>,
    do_parse!(
        program: ws!(alpha) >>
        args: many0!(ws!(is_not!(" &<>|"))) >>
        redirect_from: opt!(complete!(parse_redirect_from)) >>
        redirect_to: opt!(complete!(parse_redirect_to)) >>

        (Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            stdin: redirect_from.unwrap_or(StdX::StdIn),
            stdout: redirect_to.unwrap_or(StdX::StdOut),
            pid: 0,
            status: Status::Running,
        })
    )
);


#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    program: String,
    args: Vec<String>,
    stdin: StdX,
    stdout: StdX,
    pub pid: libc::pid_t,
    status: Status,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Completed,
    Running,
    Suspended,
    Terminated,
}


impl Command {

    pub fn new(program: &str) -> Self {
        Command {
            program: program.to_string(),
            args: vec![],
            stdin: StdX::StdIn,
            stdout: StdX::StdOut,
            pid: 0,
            status: Status::Running,
        }
    }

    pub fn arg(&mut self, a: &str) -> &mut Self {
        self.args.push(a.to_string());
        self
    }

    pub fn args(&mut self, v: Vec<&str>) -> &mut Self {
        self.args.extend(v.iter().map(|a| a.to_string()));
        self
    }

    pub fn pid(&mut self, p: libc::pid_t) -> &mut Self {
        self.pid = p;
        self
    }

    pub fn status(&mut self, s: Status) -> &mut Self {
        self.status = s;
        self
    }

    pub fn stdin(&mut self, stdx: StdX) -> &mut Self {
        match stdx {
            StdX::StdErr | StdX::StdOut => panic!(),
            _ => self.stdin = stdx,
        }
        self
    }

    pub fn stdout(&mut self, stdx: StdX) -> &mut Self {
        match stdx {
            StdX::StdErr | StdX::StdIn => panic!(),
            _ => self.stdout = stdx,
        }
        self
    }

    pub fn is_completed(&self) -> bool {
        self.status == Status::Completed
    }

    pub fn is_suspended(&self) -> bool {
        self.status == Status::Suspended
    }

    pub fn is_terminated(&self) -> bool {
        self.status == Status::Terminated
    }

    fn cprogram(&self) -> Result<CString, ffi::NulError> {
        CString::new(self.program.as_bytes())
    }

    fn cargs(&self) -> Result<Vec<CString>, ffi::NulError> {
        self.args.iter().map(|a| CString::new(a.as_bytes())).collect()
    }

    pub fn parse(command: &str) -> Option<Self> {
        match parse_command(command.trim()) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    pub fn spawn(&self, pgid: libc::pid_t) -> io::Result<libc::pid_t> {
        match fork()? {

            ForkResult::Child => {

                Signal::default();
                setpgid(0, pgid)?;

                match self.stdin {
                    StdX::Pipe(ref fd) => dupclose(*fd, libc::STDIN_FILENO)?,
                    StdX::Redirect(ref path) => dupclose(File::open(path)?.into_raw_fd(), libc::STDIN_FILENO)?,
                    _ => (),
                }

                match self.stdout {
                    StdX::Pipe(ref fd) => dupclose(*fd, libc::STDOUT_FILENO)?,
                    StdX::Redirect(ref path) => dupclose(File::create(path)?.into_raw_fd(), libc::STDOUT_FILENO)?,
                    _ => (),
                }

                let mut args = vec![self.cprogram()?];
                args.extend(self.cargs()?);

                if let Err(e) = execvp(&args[0], &args) {
                    println!("{:?}", e.errno().desc()); // FIXME: write to stderr
                    unsafe { libc::exit(0); }
                }

                panic!();
            },

            ForkResult::Parent { child } => {
                setpgid(child, pgid)?; // FIXME: if errno = EACCES, this is not an error
                Ok(child)
            },

        }
    }

}


fn dupclose(from: RawFd, to: RawFd) -> Result<(), nix::Error> {
    dup2(from, to)?;
    close(from)
}


#[test]
fn test_command_new() {
    assert_eq!(
        Command::parse("ls"),
        Some(Command::new("ls"))
    );

    assert_eq!(
        Command::parse("ls -la"),
        Some(Command::new("ls").arg("-la").to_owned())
    );

    assert_eq!(
        Command::parse("rm -rf dir"),
        Some(Command::new("rm").args(vec!["-rf", "dir"]).to_owned())
    );

    assert_eq!(
        Command::parse("ls -la > output.txt"),
        Some(Command::new("ls")
                .arg("-la")
                .stdout(StdX::Redirect("output.txt".to_string())).to_owned())
    );

    assert_eq!(
        Command::parse("sort -r < input.txt"),
        Some(Command::new("sort")
                .arg("-r")
                .stdin(StdX::Redirect("input.txt".to_string())).to_owned())
    );

    assert_eq!(
        Command::parse("sort -r < input.txt > output.txt"),
        Some(Command::new("sort")
                .arg("-r")
                .stdin(StdX::Redirect("input.txt".to_string()))
                .stdout(StdX::Redirect("output.txt".to_string())).to_owned())
    );
}

#[test]
fn test_status() {
    assert!(!Command::new("ls").is_completed());
    assert!(Command::new("ls").status(Status::Completed).is_completed());
}
