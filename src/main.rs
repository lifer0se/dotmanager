use color_print::{cformat, cprintln};
use crossterm::{
    cursor,
    terminal::{self, disable_raw_mode, enable_raw_mode},
    ExecutableCommand, QueueableCommand,
};
use dialoguer::{
    console::{style, Key, Term},
    theme::ColorfulTheme,
    Select,
};
use prettytable::{Cell, Row, Table};
use std::{
    cmp::max,
    env,
    fs::{self, metadata},
    io::{stdout, Write},
    process::{exit, Command, Stdio},
};

mod util;
use util::functions::{
    file_to_vec, new_table, print_path_error, read_input, sanitise_args, split_cmd, validate_args,
};
use util::user_paths::{GIT, HOME, LIST};
use util::StatusInfo;

fn main() {
    let valid_inputs = vec![
        "hslud;i:a:r:",
        "help, status, status-summary, list, update, diff;, init:, add:, remove:",
    ];
    handle_input(&valid_inputs);
}

fn handle_input(valid_inputs: &[&str]) {
    let args: &Vec<String> = &env::args().collect();
    if args.len() <= 1 || args.len() > 3 {
        help();
        exit(2);
    }

    let sargs: (String, String) = sanitise_args(args);
    if !validate_args(&sargs, valid_inputs) {
        help();
        exit(2);
    }

    match sargs.0.as_str() {
        "u" | "update" => update(),
        "s" | "status" => status(),
        "status-summary" => status_summary_short(),
        "l" | "list" => list(),
        "d" | "diff" => diff(&sargs.1),
        "i" | "init" => init(&sargs.1),
        "a" | "add" => add(&sargs.1),
        "r" | "remove" => remove(&sargs.1),
        "h" | "help" => {
            help();
            exit(0);
        }
        _ => {
            help();
            exit(2);
        }
    }
}

fn help() {
    cprintln!(
"
<bold>Dotmanager</> is a utility that creates and maintains a bare git repository to manage dotfiles.

<green,bold>Usage</>: <cyan><bold>dm</bold> [option] (argument)</>

<green,bold>Options</>:
<cyan,bold>  -h</>, <cyan,bold>--help</>           Displays the help message.
<cyan,bold>  -s</>, <cyan,bold>--status</>         Displays the status of the dotfile repository.
<cyan,bold>  -l</>, <cyan,bold>--list</>           Displays the tracking list.
<cyan,bold>  -i</>, <cyan><bold>--init</bold> <<url>></>     Initializes a bare git repository under $XDG_DATA_HOME/dotmanager and does an initial commit and push to the remote-url.
<cyan,bold>  -u</>, <cyan,bold>--update</>         Stages all changes of folders and files in the tracking list, then prompts the user for commit & push.
<cyan,bold>  -a</>, <cyan><bold>--add</bold> <<path>></>     Adds a file or folder to the tracking list and stages the change.
<cyan,bold>  -r</>, <cyan><bold>--remove</bold> <<path>></>  Removes a file or folder from the tracking list and stages the change.
<cyan,bold>  -d</>, <cyan><bold>--diff</bold> (<<file>>)</>  Displays git diff. Comparing the latest commit with the live work-tree. Without an argument, shows a list of all diff files.
"
);
}

fn update() {
    git_add_all();
    println!();
    let status_info = get_status_info();
    println!("{}", status_info.work_tree);
    println!("{}", status_info.remote_url);
    println!("{}", status_info.status);
    if !status_info.entry_type_counts.is_empty() {
        status_info.table.printstd();
        select_next_step(&status_info);
    }
}

fn status() {
    git_add_all();
    println!();
    let status_info = get_status_info();
    println!("{}", status_info.work_tree);
    println!("{}", status_info.remote_url);
    println!("{}", status_info.status);
    if !status_info.entry_type_counts.is_empty() {
        status_info.table.printstd();
        println!("{}", status_info.summary);
    }
}

fn status_summary_short() {
    git_add_all();
    let status = git_command_output("status --porcelain");
    if !status.is_empty() {
        let status_lines: Vec<String> = status.trim().split('\n').map(|l| l.to_string()).collect();
        let status_counts = get_status_counts(&status_lines);
        println!("{}", get_status_summary_short(&status_counts));
    }
}

fn list() {
    git_add_all();
    println!();
    print_tracking_list_table();
    println!();
}

fn diff(file: &str) {
    git_add_all();
    if file.is_empty() {
        let status_info = get_status_info();
        status_info.table.printstd();
        diff_file_select(&status_info);
    } else {
        diff(file);
    }
}

fn init(repo_url: &String) {
    if metadata(GIT.as_str()).is_err() {
        fs::create_dir_all(GIT.as_str()).expect("Could not create git data directory");
    }
    let home_git_path = format!("{}/.github", HOME.as_str());
    if metadata(&home_git_path).is_err() {
        fs::create_dir(&home_git_path).expect("Could not create $HOME/.github");
    }
    let readme_path = format!("{}/.github/README.md", HOME.as_str());
    if metadata(&readme_path).is_err() {
        fs::File::create(&readme_path).expect("Could not create $HOME/.github/README.md");
    }
    git_command_output(format!("git init --bare {}", GIT.as_str()).as_str());
    git_command_output("config --local status.showUntrackedFiles no");
    git_command_output("branch -M main");
    git_command_output(format!("remote add origin {repo_url}").as_str());
    git_command_output(format!("add {readme_path}").as_str());
    git_command_output("commit -m \"Initial commit\"");
    git_command_output("push -u origin main");
}

fn add(path: &String) {
    check_path_exists(path);
    add_to_tracking_list(path);
    git_command_spawn(format!("add {path}").as_str());
}

fn remove(path: &String) {
    check_path_exists(path);
    remove_from_tracking_list(path);
    git_command_spawn(format!("rm -rf {path}").as_str());
}

fn git_add_all() {
    let mut paths = file_to_vec(LIST.as_str());
    let l = paths.len();
    paths.retain(|p| metadata(p).is_ok());
    if l != paths.len() {
        fs::write(LIST.as_str(), paths.join("\n")).expect("Could not update path list.");
    }
    for path in paths {
        git_command_spawn(format!("add {path}").as_str());
    }
}

fn get_status_info() -> StatusInfo {
    let mut status_info = StatusInfo::default();
    let url = git_command_output("remote get-url --all origin");
    let status_output = git_command_output("status --porcelain");

    status_info.work_tree = cformat!(" <bold>{}</>\t<cyan>{}/</>", "Work-tree:", HOME.as_str());
    status_info.remote_url = cformat!(" <bold>{}</>\t<cyan>{}</>", "Remote-URL:", url.trim());
    if !status_output.is_empty() {
        status_info.status = cformat!(" <bold>Git status:</>");
        let status_lines: Vec<String> = status_output
            .trim()
            .split('\n')
            .map(|l| l.to_string())
            .collect();

        status_info.entry_type_counts = get_status_counts(&status_lines);
        status_info.table = get_status_table(&status_lines, &mut status_info);
        status_info.summary = get_status_summary(&status_info.entry_type_counts);
        status_info.summary_short = get_status_summary_short(&status_info.entry_type_counts);
    } else {
        status_info.status = cformat!(" <bold>Git status:\t<green>Up to date</>");
    }
    status_info
}

fn select_next_step(status_info: &StatusInfo) {
    let options = ["commit & push", "diff", "exit"];
    let theme = ColorfulTheme {
        prompt_prefix: style("".to_string()).for_stderr().yellow(),
        prompt_suffix: style("".to_string()).for_stderr().black().bright(),
        ..Default::default()
    };

    let selection = Select::with_theme(&theme)
        .with_prompt("Proceed to:")
        .default(0)
        .items(&options[..])
        .interact()
        .unwrap();

    let mut stdout = stdout();
    stdout.queue(cursor::MoveUp(1)).unwrap();
    stdout.execute(terminal::Clear(terminal::ClearType::CurrentLine)).unwrap();

    match selection {
        0 => {
            commit_and_push();
        }
        1 => {
            diff_file_select(status_info);
            select_next_step(status_info);
        }
        _ => {
            println!("Terminating.");
            exit(2);
        }
    }
}

fn commit_and_push() {
    let message = read_input("Add commit message: ");
    git_command_spawn(format!("commit -m \"{message}\"").as_str());
    git_command_spawn("push");
}

fn check_path_exists(path: &String) {
    if metadata(path.trim_end_matches('/')).is_err() {
        print_path_error("error", "did not match any files or folders", path);
        exit(2);
    }
}

fn diff_file(file: &str) {
    let cached_diff = git_command_output("diff --cached");
    let cached_diff = cached_diff.split("diff --git ").collect::<Vec<&str>>();
    let diff_paths = git_command_output("diff --cached --name-only");
    let diff_paths = diff_paths.split('\n').collect::<Vec<&str>>();
    let file = file.trim_start_matches((HOME.to_string() + "/").as_str());
    let file_diff: Vec<&str> = match diff_paths
        .into_iter()
        .enumerate()
        .find(|(_i, p)| &file == p)
    {
        Some((i, _p)) => cached_diff[i + 1].split('\n').collect(),
        None => {
            print_path_error("warn", "did not find any changes", &file.to_string());
            exit(0);
        }
    };

    let mut output = cformat!("<bold>diff --git </>");
    let mut found_atat = false;
    for line in file_diff {
        if line.starts_with("@@ ") {
            found_atat = true;
            let split = line.split("@@").collect::<Vec<&str>>();
            output += &cformat!("<cyan>@@{}@@</>{}\n", split[1], split[2]);
            continue;
        }
        if !found_atat {
            output += &cformat!("<bold>{line}</>\n");
            continue;
        }

        if line.starts_with('+') {
            output += &cformat!("<green>{line}</>\n");
        } else if line.starts_with('-') {
            output += &cformat!("<red>{line}</>\n");
        } else {
            output += &cformat!("{line}\n");
        }
    }

    let mut child = Command::new("less")
        .arg("-~")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(output.as_bytes())
        .expect("Could not pipe to less");
    drop(stdin);
    child.wait().expect("wait for less failed");
}

fn diff_file_select(status_info: &StatusInfo) {
    let names = git_command_output("diff --cached --name-only");
    if names.is_empty() {
        println!("There are no modified files. Git appears to be up to date");
        println!("Terminating");
        exit(0);
    }

    let mut stdout = stdout();
    stdout.execute(cursor::Hide).unwrap();
    enable_raw_mode().unwrap();

    let mut index: i32 = 0;
    select_status_entry(status_info, &index, true);
    loop {
        if let Ok(c) = Term::buffered_stdout().read_key() {
            match c {
                Key::Tab | Key::ArrowDown => {
                    next_status_entry(status_info, &mut index, 1);
                }
                Key::BackTab | Key::ArrowUp => {
                    next_status_entry(status_info, &mut index, -1);
                }
                Key::Enter => {
                    diff_file(&status_info.status_entries[index as usize].1.trim()[10..]);
                }
                _ => {
                    select_status_entry(status_info, &index, false);
                    disable_raw_mode().unwrap();
                    stdout.execute(cursor::Show).unwrap();
                    break;
                }
            }
        }
    }
}

fn next_status_entry(status_info: &StatusInfo, index: &mut i32, dir: i32) {
    select_status_entry(status_info, index, false);
    let l = status_info.status_entries.len() as i32;
    *index = (*index + dir) % l;
    if *index < 0 {
        *index += l;
    }
    select_status_entry(status_info, index, true);
}

fn select_status_entry(status_info: &StatusInfo, index: &i32, select: bool) {
    let size0 = status_info
        .status_entries
        .iter()
        .map(|e| e.0.len())
        .max()
        .unwrap();
    let size1 = status_info
        .status_entries
        .iter()
        .map(|e| e.1.len())
        .max()
        .unwrap();
    let path = &status_info.status_entries[*index as usize].1;
    let start: u16 = (status_info.status_entries.len() as i32 - index + 1) as u16;
    let right: u16 = (size0 + 5) as u16;

    let mut stdout = stdout();
    stdout.queue(cursor::SavePosition).unwrap();
    stdout.queue(cursor::MoveToPreviousLine(start)).unwrap();
    stdout.queue(cursor::MoveToColumn(right)).unwrap();

    let selection = if select {
        cformat!("<cyan>{}</>", path)
    } else {
        path.clone()
    };
    stdout.write_all(selection.as_bytes()).unwrap();

    stdout
        .queue(cursor::MoveRight((size1 - path.len() + 3) as u16))
        .unwrap();
    let selection = if select {
        cformat!("<green>❮</>")
    } else {
        cformat!(" ")
    };
    stdout.write_all(selection.as_bytes()).unwrap();

    stdout.queue(cursor::RestorePosition).unwrap();
    stdout.flush().unwrap();
}

fn add_to_tracking_list(path: &String) {
    let mut paths = file_to_vec(LIST.as_str());
    for p in paths.iter() {
        if path == p {
            print_path_error("warn", "is already in the tracking list", path);
            exit(2);
        }
        if path.contains(p) {
            print_path_error(
                "warn",
                format!("entry exists at lower depth: '{p}'").as_str(),
                path,
            );
            exit(2);
        }
    }
    paths.retain(|p| !p.contains(path));
    paths.push(path.clone());
    fs::write(LIST.as_str(), paths.join("\n")).expect("Could not write new path to list.");
}

fn remove_from_tracking_list(path: &String) {
    let mut paths = file_to_vec(LIST.as_str());
    let size = paths.len();
    paths.retain(|p| p != path);
    if paths.len() == size {
        print_path_error(
            "error",
            "did not match any files or folders in the tracking list",
            path,
        );
        exit(2);
    }
    fs::write(LIST.as_str(), paths.join("\n")).expect("Could not write new path to list.");
}

fn get_status_table(status_lines: &[String], status_info: &mut StatusInfo) -> Table {
    let mut table = new_table();
    table.set_titles(Row::new(vec![
        Cell::new("status").style_spec("bFgc"),
        Cell::new("path").style_spec("bFgc"),
    ]));

    let matches = ["A", "D", "M"];
    let titles = ["new file", "deleted", "modified"];
    let specs = ["Fb", "Fr", "Fg"];
    for line in status_lines.iter().cloned() {
        for ((m, title), spec) in matches.iter().zip(titles.iter()).zip(specs.iter()) {
            if !line[0..2].contains(m) {
                continue;
            }
            let mut path = line
                .split(' ')
                .collect::<Vec<&str>>()
                .last()
                .unwrap()
                .to_string();
            path = cformat!("<dim>/</>{}", path);
            status_info
                .status_entries
                .push((title.to_string(), path.clone()));
            table.add_row(Row::new(vec![
                Cell::new(title).style_spec(spec),
                Cell::new(path.as_str()),
            ]));
        }
    }
    table
}

fn print_tracking_list_table() {
    let mut files: Vec<&str> = vec![];
    let mut folders: Vec<&str> = vec![];
    let paths = file_to_vec(LIST.as_str());
    for path in paths.iter() {
        let md = metadata(path).unwrap();
        if md.is_dir() {
            folders.push(&path[HOME.as_str().len() + 1..]);
        } else {
            files.push(&path[HOME.as_str().len() + 1..]);
        }
    }
    folders.sort();
    files.sort();

    let mut table = new_table();
    table.set_titles(Row::new(vec![
        Cell::new("#").style_spec("bFgr"),
        Cell::new("folders").style_spec("bFgc"),
        Cell::new("files").style_spec("bFgc"),
    ]));

    for i in 0..max(files.len(), folders.len()) {
        let mut folder = String::new();
        let mut file = String::new();
        if i < folders.len() {
            folder = cformat!(" <dim>/</>{}", folders[i]);
        }
        if i < files.len() {
            file = cformat!(" <dim>/</>{}", files[i]);
        }

        table.add_row(Row::new(vec![
            Cell::new((i + 1).to_string().as_str()).style_spec("bFgr"),
            Cell::new(folder.as_str()).style_spec("Fb"),
            Cell::new(file.as_str()),
        ]));
    }

    cprintln!("<bold> Tracking:</>");
    table.printstd();
}

fn get_status_summary_short(status_counts: &[i32]) -> String {
    let titles = ["+", "-", "~"];
    let mut status_summary = String::new();
    for (c, t) in status_counts.iter().zip(titles.iter()) {
        if c > &0 {
            status_summary += format!("{t}{c} ").as_str();
        }
    }
    status_summary
}

fn get_status_summary(status_counts: &[i32]) -> String {
    let colors = ["\u{1b}[34m", "\u{1b}[31m", "\u{1b}[32m"]; // [ blue, red, green ]
    let titles = ["new files: ", "deleted: ", "modified: "];
    let results: Vec<String> = status_counts
        .iter()
        .zip(colors.iter())
        .zip(titles.iter())
        .map(|((count, color), title)| {
            if count > &0 {
                let count = format!("{}{}{}", color, count, "\u{1b}[0m");
                cformat!(" <dim>></> {}{}\n", title, count)
            } else {
                String::from("")
            }
        })
        .collect();

    let mut output = String::new();
    for r in results {
        output += r.as_str();
    }
    output.trim_end().to_string()
}

fn get_status_counts(status_lines: &[String]) -> Vec<i32> {
    let mut counts: Vec<i32> = vec![];
    let matches = ["A", "D", "M"];
    for m in matches {
        counts.push(status_lines.iter().filter(|l| l[0..2].contains(m)).count() as i32);
    }
    counts
}

fn git_command_output(cmd: &str) -> String {
    let args = format!(
        "--git-dir={} --work-tree={} {}",
        GIT.as_str(),
        HOME.as_str(),
        cmd
    );
    let split = split_cmd(args);
    let output = Command::new("/bin/git")
        .args(split)
        .output()
        .expect("failed to execute process");

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn git_command_spawn(cmd: &str) {
    let args = format!(
        "--git-dir={} --work-tree={} {}",
        GIT.as_str(),
        HOME.as_str(),
        cmd
    );
    let split = split_cmd(args);
    Command::new("/bin/git")
        .args(split)
        .spawn()
        .expect("failed to spawn process")
        .wait()
        .expect("failed to execute process");
}
