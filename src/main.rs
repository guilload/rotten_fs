#[macro_use]
extern crate nom;

use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::os::unix::io::{IntoRawFd, FromRawFd};
use std::process;

use nom::*;


#[derive(Debug, PartialEq)]
enum StdX {
    Redirect(String),
    StdErr,
    StdIn,
    StdOut,
}

named!(parse_redirect_to<&str, StdX>,
    do_parse!(
        tag!(">") >>
        path: ws!(is_not!(" ")) >>

        (StdX::Redirect(path.to_string()))
    )
);

named!(parse_redirect_from<&str, StdX>,
    do_parse!(
        tag!("<") >>
        path: ws!(is_not!(" >")) >>

        (StdX::Redirect(path.to_string()))
    )
);

named!(parse_command<&str, Command>,
    do_parse!(
        program: ws!(alpha) >>
        args: many0!(ws!(is_not!(" <>"))) >>
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


#[derive(Debug, PartialEq)]
struct Command {
    program: String,
    args: Vec<String>,
    stdin: StdX,
    stdout: StdX,
}


impl Command {

    fn new(command: &str) -> Option<Self> {
        match parse_command(command.trim()) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    fn run(&self) -> io::Result<()> {
        process::Command::new(&self.program)
            .args(&self.args)
            .stdout(self.stdout()?)
            .spawn()?
            .wait()?;

        Ok(())
    }

    fn stdout(&self) -> io::Result<process::Stdio> {
        let stdio = match &self.stdout {
            &StdX::Redirect(ref path) => File::create(path)?.into_stdio(),
            _ => process::Stdio::inherit(),
        };
        Ok(stdio)
    }

}

trait IntoStdio {

    fn into_stdio(self) -> process::Stdio;
}

impl IntoStdio for File {

    fn into_stdio(self) -> process::Stdio {
        let fd = self.into_raw_fd();
        unsafe { process::Stdio::from_raw_fd(fd) }
    }

}

fn main() {
    println!("Rotten sh...");

    loop {
        print!("$ ");
        std::io::stdout().flush();

        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer);

        if let Some(cmd) = Command::new(&buffer) {
            if let Err(e) = cmd.run() {
                println!("Command failed: {:?}", e);
            }
        }
    }
}


#[test]
fn test_command_new() {
    assert_eq!(
        Command::new("ls"),
        Some(Command {
            program: "ls".to_string(),
            args: vec![],
            stdin: StdX::StdIn,
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("ls -la"),
        Some(Command {
            program: "ls".to_string(),
            args: vec!["-la".to_string()],
            stdin: StdX::StdIn,
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("rm -rf dir"),
        Some(Command {
            program: "rm".to_string(),
            args: vec!["-rf".to_string(), "dir".to_string()],
            stdin: StdX::StdIn,
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("ls -la > output.txt"),
        Some(Command {
            program: "ls".to_string(),
            args: vec!["-la".to_string()],
            stdin: StdX::StdIn,
            stdout: StdX::Redirect("output.txt".to_string()),
        })
    );

    assert_eq!(
        Command::new("sort -r < input.txt"),
        Some(Command {
            program: "sort".to_string(),
            args: vec!["-r".to_string()],
            stdin: StdX::Redirect("input.txt".to_string()),
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("sort -r < input.txt > output.txt"),
        Some(Command {
            program: "sort".to_string(),
            args: vec!["-r".to_string()],
            stdin: StdX::Redirect("input.txt".to_string()),
            stdout: StdX::Redirect("output.txt".to_string()),
        })
    );
}
