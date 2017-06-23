#[macro_use]
extern crate nom;

use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::os::unix::io::{IntoRawFd, FromRawFd};
use std::process;

use nom::*;


#[derive(Debug, Clone, PartialEq)]
enum StdX {
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

    fn run(&self) -> io::Result<()> {
        Ok(())
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

    fn parse(command: &str) -> Option<Self> {
        match parse_command(command.trim()) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    fn run(&self) -> io::Result<()> {
        process::Command::new(&self.program)
            .args(&self.args)
            .stdin(self.fdin()?)
            .stdout(self.fdout()?)
            .spawn()?
            .wait()?;

        Ok(())
    }

    fn fdin(&self) -> io::Result<process::Stdio> {
        let stdio = match &self.stdin {
            &StdX::Redirect(ref path) => File::open(path)?.into_stdio(),
            _ => process::Stdio::inherit(),
        };
        Ok(stdio)
    }

    fn fdout(&self) -> io::Result<process::Stdio> {
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
