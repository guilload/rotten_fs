#![feature(libc)]
extern crate libc;

extern crate nix;
extern crate nom;

use std::ffi::CString;
use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::os::unix::io::{IntoRawFd, RawFd};

use nix::sys::wait::waitpid;
use nix::unistd::{close, dup2, execvp, fork, ForkResult, pipe};
use nom::*;


fn dupclose(from: RawFd, to: RawFd) -> Result<(), nix::Error> {
    dup2(from, to)?;
    close(from)
}

#[derive(Debug, Clone, PartialEq)]
enum StdX {
    Pipe(RawFd),
    Redirect(String),
    StdErr,
    StdIn,
    StdOut,
}

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

named!(parse_command<&str, Command>,
    do_parse!(
        program: ws!(alpha) >>
        args: many0!(ws!(is_not!(" <>|"))) >>
        redirect_from: opt!(complete!(parse_redirect_from)) >>
        redirect_to: opt!(complete!(parse_redirect_to)) >>

        (Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            stdin: redirect_from.unwrap_or(StdX::StdIn),
            stdout: redirect_to.unwrap_or(StdX::StdOut),
        })
    )
);

named!(parse_pipeline<&str, Pipeline>,
    do_parse!(
        init: parse_command >>
        commands:
            fold_many0!(
                do_parse!(
                    tag!("|") >>
                    command: ws!(parse_command) >>
                    (command)
                ),
                vec![init],
                |mut acc: Vec<Command>, cmd| { acc.push(cmd); acc }
            ) >>

        (Pipeline { commands: commands } )
    )
);

#[derive(Debug, PartialEq)]
struct Pipeline {
    commands: Vec<Command>,
}

impl Pipeline {

    fn parse(pipeline: &str) -> Option<Self> {
        match parse_pipeline(pipeline.trim()) {
            IResult::Done(_, ppln) => Some(ppln),
            _ => None
        }
    }

    fn spawn(&mut self) -> io::Result<Vec<libc::pid_t>> {
        let mut pastfdin: Option<RawFd> = None;
        let mut pids = vec![];

        for i in 0..(self.commands.len() - 1) {
            let (fdin, fdout) = pipe()?;

            self.commands[i].stdout(StdX::Pipe(fdout));
            self.commands[i + 1].stdin(StdX::Pipe(fdin));

            let pid = self.commands[i].spawn()?;
            pids.push(pid);

            match pastfdin { // closing fdin from past iteration
                Some(fd) => close(fd)?,
                _ => (),
            }

            pastfdin = Some(fdin);
            close(fdout);
        }

        pids.push(self.commands.last().unwrap().spawn()?);

        match pastfdin { // and here again...
                Some(fd) => close(fd)?,
                _ => (),
            }

        Ok(pids)
    }

}


#[derive(Debug, Clone, PartialEq)]
struct Command {
    program: String,
    args: Vec<String>,
    stdin: StdX,
    stdout: StdX,
}


impl Command {

    fn new(program: &str) -> Self {
        Command {
            program: program.to_string(),
            args: vec![],
            stdin: StdX::StdIn,
            stdout: StdX::StdOut,
        }
    }

    fn arg(&mut self, a: &str) -> &mut Self {
        self.args.push(a.to_string());
        self
    }

    fn args(&mut self, v: Vec<&str>) -> &mut Self {
        self.args.extend(v.iter().map(|a| a.to_string()));
        self
    }

    fn stdin(&mut self, stdx: StdX) -> &mut Self {
        match stdx {
            StdX::StdErr | StdX::StdOut => panic!(),
            _ => self.stdin = stdx,
        }
        self
    }

    fn stdout(&mut self, stdx: StdX) -> &mut Self {
        match stdx {
            StdX::StdErr | StdX::StdIn => panic!(),
            _ => self.stdout = stdx,
        }
        self
    }

    fn cprogram(&self) -> Result<CString, std::ffi::NulError> {
        CString::new(self.program.as_bytes())
    }

    fn cargs(&self) -> Result<Vec<CString>, std::ffi::NulError> {
        self.args.iter().map(|a| CString::new(a.as_bytes())).collect()
    }

    fn parse(command: &str) -> Option<Self> {
        match parse_command(command.trim()) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    fn spawn(&self) -> io::Result<libc::pid_t> {
        match fork()? {

            ForkResult::Child => {

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

                execvp(&args[0], &args)?;

                Ok(0)
            },

            ForkResult::Parent { child } => Ok(child),
        }
    }

}


fn main() {
    println!("Rotten sh...");

    loop {
        print!("$ ");
        std::io::stdout().flush();

        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer);

        if let Some(mut pipeline) = Pipeline::parse(&buffer) {
            let pids = pipeline.spawn();

            while waitpid(-1, None).is_ok() {}

            if let Err(e) = pids {
                    println!("Command failed: {:?}", e);
            };
        }
    }
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
fn test_pipeline_new() {
    assert_eq!(
        Pipeline::parse("ls"),
        Some(
            Pipeline {
                commands: vec![Command::new("ls")]
            }
        )
    );

    assert_eq!(
        Pipeline::parse("ls | wc"),
        Some(
            Pipeline {
                commands: vec![Command::new("ls"), Command::new("wc")]
            }
        )
    );
}
