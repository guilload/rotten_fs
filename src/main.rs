#[macro_use]
extern crate nom;

use std::io::prelude::*;
use std::io;
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

named!(parse_command<&str, Command>,
    do_parse!(
        program: ws!(alpha) >>
        args: many0!(ws!(is_not!(" >"))) >>
        redirect_to: opt!(complete!(parse_redirect_to)) >>

        (Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            stdout: redirect_to.unwrap_or(StdX::StdOut),
        })
    )
);


#[derive(Debug, PartialEq)]
struct Command {
    program: String,
    args: Vec<String>,
    stdout: StdX,
}


impl Command {

    fn new(command: &str) -> Option<Self> {
        match parse_command(command) {
            IResult::Done(_, cmd) => Some(cmd),
            _ => None
        }
    }

    fn run(&self) -> io::Result<()> {
        process::Command::new(&self.program)
            .args(&self.args)
            .spawn()?
            .wait()?;

        Ok(())
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
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("ls -la"),
        Some(Command {
            program: "ls".to_string(),
            args: vec!["-la".to_string()],
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("rm -rf dir"),
        Some(Command {
            program: "rm".to_string(),
            args: vec!["-rf".to_string(), "dir".to_string()],
            stdout: StdX::StdOut,
        })
    );

    assert_eq!(
        Command::new("ls -la > foo.txt"),
        Some(Command {
            program: "ls".to_string(),
            args: vec!["-la".to_string()],
            stdout: StdX::Redirect("foo.txt".to_string()),
        })
    );
}
