use prettytable::Table;


pub struct StatusInfo {
    pub work_tree: String,
    pub remote_url: String,
    pub status: String,
    pub status_lines: Vec<String>,
    pub table: Table,
    pub summary: String,
    pub summary_short: String,
}

impl Default for StatusInfo {
    fn default() -> Self {
        StatusInfo {
            work_tree: String::new(),
            remote_url: String::new(),
            status: String::new(),
            status_lines: vec![],
            table: Table::new(),
            summary: String::new(),
            summary_short: String::new(),
        }
    }
}

pub mod user_paths {

    use dirs::{data_dir, home_dir};
    use once_cell::sync::Lazy;
    use std::process::exit;

    pub static HOME: Lazy<String> = Lazy::new(|| match home_dir() {
        Some(p) => p.into_os_string().into_string().unwrap(),
        None => {
            println!("Could not find $HOME");
            exit(2);
        }
    });

    static DATA: Lazy<String> = Lazy::new(|| match data_dir() {
        Some(mut p) => {
            p.push("dotmanager");
            p.into_os_string().into_string().unwrap()
        }
        None => {
            println!("Could not find DATA directory");
            exit(2);
        }
    });

    pub static GIT: Lazy<String> = Lazy::new(|| {
        let mut git = DATA.to_string();
        git.push_str("/git");
        git
    });

    pub static LIST: Lazy<String> = Lazy::new(|| {
        let mut list = DATA.to_string();
        list.push_str("/list");
        list
    });
}

pub mod functions {

    use prettytable::{format, Table};
    use std::io::Write;
    use std::{fs, io};

    pub fn new_table() -> Table {
        let mut table = Table::new();
        let format = format::FormatBuilder::new()
            .column_separator('│')
            .borders('│')
            .separators(
                &[format::LinePosition::Top],
                format::LineSeparator::new('─', '┬', '┌', '┐'),
            )
            .separators(
                &[format::LinePosition::Title],
                format::LineSeparator::new('─', '┼', '├', '┤'),
            )
            .separators(
                &[format::LinePosition::Bottom],
                format::LineSeparator::new('─', '┴', '└', '┘'),
            )
            .padding(1, 1)
            .build();
        table.set_format(format);

        table
    }

    pub fn read_input(input_message: &str) -> String {
        print!("{input_message}");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_n) => input.trim().to_lowercase(),
            Err(error) => {
                eprintln!("error: {error}");
                String::new()
            }
        }
    }

    pub fn validate_args(args: &(String, String), valid_inputs: &Vec<&str>) -> bool {
        let mut valid_input_split: Vec<String> = vec![];
        for c in valid_inputs[0].chars() {
            let mut s = c.to_string();
            if s == ":" {
                let l = valid_input_split.pop().unwrap();
                s = format!("{}{}", l, s);
            }
            valid_input_split.push(s);
        }
        let long_input_split = valid_inputs[1].split(",");
        for input in long_input_split {
            valid_input_split.push(input.trim().to_string());
        }

        let mut matched = false;
        let mut requires_input = false;
        for mut input in valid_input_split {
            requires_input = false;
            if input.ends_with(";") {
                input = input[0..input.len() - 1].to_string();
            } else if input.ends_with(":") {
                input = input[0..input.len() - 1].to_string();
                requires_input = true;
            }
            if args.0 == input {
                matched = true;
                break;
            }
        }
        !(!matched || (requires_input && args.1 == ""))
    }

    pub fn sanitise_args(args: &Vec<String>) -> (String, String) {
        let mut j = 1;
        let mut cmd: String = args[1].trim().to_string();
        if cmd.len() > 3 {
            j = 2;
        }
        cmd = cmd[j..cmd.len()].to_string();

        let mut arg: String = String::new();
        if args.len() == 3 {
            arg = args[2].trim().to_string();
        }

        (cmd, arg)
    }

    pub fn print_path_error(msgtype: &str, msg: &str, path: &String) {
        let red = "\u{1b}[31m";
        let yellow = "\u{1b}[33m";
        let bold = "\u{1b}[1m";
        let end = "\u{1b}[0m";
        let color = if msgtype == "error" { red } else { yellow };
        let msg_type = format!("{}{}{}{}", color, bold, msgtype, end);
        println!("{}{}:{} '{}': {}", msg_type, bold, end, path, msg);
    }

    pub fn split_cmd(cmd: String) -> Vec<String> {
        let mut split = vec![String::new()];
        let mut index = 0;
        let mut in_quotes = false;
        for c in cmd.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
            } else if c == ' ' && !in_quotes {
                index += 1;
                split.push(String::new());
                continue;
            }
            split[index] = format!("{}{}", split[index], c);
        }
        split
    }

    pub fn file_to_vec(file: &str) -> Vec<String> {
        let read = fs::read_to_string(file).expect("err");
        let paths: Vec<String> = read.trim().split('\n').map(|p| p.to_string()).collect();
        paths
    }

    // pub fn clear_screen() {
    //     print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    // }
}
