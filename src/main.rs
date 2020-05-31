use clap::clap_app;
use clap::AppSettings;
use scan_fmt::scan_fmt;

use dirs;
use regex::Regex;
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{BufRead, BufReader};
use std::io::prelude::*;
use std::path::{Path, PathBuf};


mod lib;

const ROOT_DIR: &str = ".alienv";
const ENV_VAR: &str  = "ALIAS_ENV";
const NO_ENV_ACTIVE: &str = "NO ENV";

const ALIAS_FILE: &str = "aliases";

const VALID_ENV_REGEX: &str = "[-_.A-Za-z0-9]+";

/********/
/* Util */
/********/

fn err_clap(err: clap::Error) {
    writeln!(io::stderr(), "{}", err).expect("Unable to write");
    std::process::exit(1);
}

/* Print an error message and exit. */
fn error(msg: &str) -> ! {
    println!("echo 'Error: {}'", msg);
    std::process::exit(1);
}

fn get_root_path() -> PathBuf {
    match dirs::home_dir() {
        Some(mut path) => {
            path.push(ROOT_DIR);
            return path
        },

        None => panic!("No home directory detected")
    }
}

fn get_alias_file(env: &str) -> PathBuf {
    let mut root_dir = get_root_path();
    root_dir.push(env);
    root_dir.push(ALIAS_FILE);
    return root_dir;
}

fn err_check<T, K>(r: Result<T, K>, err: &str) -> T {
    match r {
        Err(_) => {
            error(err);
        },

        Ok(val) => val
    }
}

fn is_cur_env(env: &str) -> bool {
    match env::var_os(ENV_VAR) {
        Some(cur_env) => {
            cur_env == env
        },
        None => false
    }
}

fn add_output_line(output: &mut String, line: &str) {
    output.push_str(&format!("{};", line));
}

/* Must be a fully POSIX-compliant environment name. */
fn is_valid_env_name(env: &str) -> bool {
    let r = Regex::new(VALID_ENV_REGEX).unwrap();
    return r.is_match(env) && env != NO_ENV_ACTIVE;
}

/* Check if env exists. */
fn env_exists(env: &str) -> bool {
    let root_dir = get_root_path();
    let envs = fs::read_dir(root_dir).expect("Unable to read directory");
    for temp in envs {
        let unwrapped = temp.expect("Unable to read env")
            .file_name()
            .into_string()
            .unwrap();
        if env == unwrapped {
            return true;
        }
    }
    return false;
}

/* Delete alias from file. Return true if successful, false if doesn't exist. */
fn delete_alias_from_file(file: &str, alias: &str) -> bool {
    let search_text = format!("alias {}=", alias);

    let mut f = err_check(
        File::open(file),
        "Unable to access aliases"
    );
    let reader = BufReader::new(&f);

    /* Filter out the line containing the alias to delete. */
    let lines : Vec<String> = reader.lines()
        .map(|x| x.unwrap())
        .collect();
    let new_lines : Vec<String> = lines.clone()
        .into_iter()
        .filter_map(
            |line| if !line.contains(&search_text) { Some(line) } else { None },
        ).collect();

    /* If no line was deleted, it doesn't exist. */
    if new_lines.len() == lines.len() {
        return false;
    }

    f = err_check(
        File::create(file),
        "Unable to write to file"
    );

    /* Write filtered lines back. */
    for line in new_lines {
        err_check(f.write(&line.as_bytes()), "Unable to write to file");
        err_check(f.write(b"\n"), "Unable to write to file");
    }

    return true;
}

/* Get command to set ENV_VAR to status. */
fn set_alias_var(output: &mut String, status: &str) {
    let shell = lib::get_shell();
    let cmd : String = shell.setenv(ENV_VAR, status);
    add_output_line(output, &cmd);
}

/* Unalias all commands in an env. */
fn unalias_all(output: &mut String, env: &str) {
    /* Open alias file. */
    let alias_file = get_alias_file(&env);
    let f = err_check(
        File::open(alias_file),
        "Unable to access aliases"
    );
    let reader = BufReader::new(&f);

    /* Construct the string to unset the aliases. */
    let unset_aliases : String = reader.lines()
        .map(|x| {
            if let Ok((alias, _)) = scan_fmt!(&x.unwrap(), "alias {}={}", String, String) {
                format!("unalias {}", alias)
            } else {
                error("Invalid alias file");
            }
        })
        .collect::<Vec<String>>()
        .join(";");

    /* Add unset aliases to output string. */
    if unset_aliases != "" {
        add_output_line(output, &unset_aliases);
    }
}

/* Alias all commands in an env. */
fn alias_all(output: &mut String, env: &str) {
    let alias_file = get_alias_file(&env);
    let new_f = err_check(
        File::open(alias_file),
        "Unable to access aliases"
    );
    let reader = BufReader::new(&new_f);

    /* Construct the string to set the new aliases. */
    let set_aliases : String = reader.lines()
        .map(|x| x.unwrap())
        .collect::<Vec<String>>()
        .join(";");

    /* Add unset aliases to output string. */
    if set_aliases != "" {
        add_output_line(output, &set_aliases);
    }
}

/*********/
/* Setup */
/*********/

/* Perform any necessary initial setup. */
fn setup(output: &mut String) {
    let root_dir = get_root_path();

    /* Create root directory if necessary. */
    if !Path::exists(&root_dir) {
        err_check(
            fs::create_dir(root_dir),
            "Cannot initialize alienv - insufficient permissions?"
        );
    }

    /* Check environment variable. */
    match env::var_os(ENV_VAR) {
        /* Set on first use. */
        None => {
            set_alias_var(output, NO_ENV_ACTIVE);
        },
        _ => ()
    }
}

/***************/
/* Subcommands */
/***************/

/* Create a new alias environment and switch to it. */
fn new(output: &mut String, env: &str) {
    /* Check if env is valid name. */
    if !is_valid_env_name(env) {
        error(&format!("Not a valid environment name. Only numbers, letters, period, underscore, and hyphen allowed."));
    }

    /* Ensure env doesn't exist. */
    if env_exists(env) {
        error(&format!("Environment {} already exists.", env));
    }

    let mut root_dir = get_root_path();
    root_dir.push(env);

    /* Create dir for new env. */
    err_check(
        fs::create_dir(&root_dir),
        "Cannot create directory - insufficient permissions?"
    );

    /* Create new files. */
    let mut aliases = root_dir.clone();
    aliases.push(ALIAS_FILE);
    err_check(
        File::create(aliases),
        "Cannot create file - insufficient permissions?"
    );

    load(output, env);
}

/* Delete an alias environment. */
fn delete(output: &mut String, env: &str) {
    let mut root_dir = get_root_path();
    root_dir.push(env);

    if !Path::exists(&root_dir) {
        error(&format!("No such environment: {}", env));
    }

    /* Check environment variable. */
    if is_cur_env(env) {
        /* If deleting current env, unset all aliases and active env. */
        unalias_all(output, env);

        /* Unset active env. */
        set_alias_var(output, NO_ENV_ACTIVE);
    }

    /* Delete env directory. */
    fs::remove_dir_all(&root_dir).expect("Unable to delete environment");
}

/* Load a new environment. */
fn load(output: &mut String, env: &str) {
    /* Ensure env exists. */
    if !env_exists(env) {
        error(&format!("Environment {} does not exist.", env));
    }

    /* Unset current aliases if needed. */
    if let Some(cur_env) = env::var_os(ENV_VAR) {
        if cur_env == env {
            error("Environment already loaded");
        }

        if cur_env != NO_ENV_ACTIVE {
            /* Unset all current aliases. */
            unalias_all(output, &cur_env.into_string().unwrap());
        }
    }

    /* Set active env to new env. */
    set_alias_var(output, &env);

    /* Load aliases of new env. */
    alias_all(output, &env);
}

fn show_all(output: &mut String) {
    let root_dir = get_root_path();
    let envs = fs::read_dir(root_dir).expect("Unable to read directory");

    for dir in envs {
        let env = dir.expect("Unable to read environment")
            .file_name()
            .into_string()
            .unwrap();
        if is_cur_env(&env) {
            output.push_str(&format!("echo '{}*';", env));
        } else {
            output.push_str(&format!("echo '{}';", env));
        }
    }
}

/* Add a new alias to the current env. */
fn add_alias(output: &mut String, alias: &str, command: &str) {
    let env = match env::var_os(ENV_VAR) {
        Some(cur_env) => {
            if cur_env == NO_ENV_ACTIVE {
                error("No alias env active.");
            };
            cur_env
        },
        /* Set on first use. */
        None => {
            error(&format!("${} does not exist. Rerun setup.", ENV_VAR))
        }
    }.into_string().unwrap();

    /* Add new alias to file. */
    let alias_file = get_alias_file(&env);
    let new_alias = format!("alias {}=\"{}\"", alias, command);
    
    let mut f = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&alias_file)
        .unwrap();

    err_check(
        writeln!(f, "{}", &new_alias),
        "Unable to write alias."
    );

    /* Set new alias. */
    add_output_line(output, &new_alias);
}

/* Remove an alias from the current env. */
fn remove_alias(output: &mut String, alias: &str) {
    let env = match env::var_os(ENV_VAR) {
        Some(cur_env) => {
            if cur_env == NO_ENV_ACTIVE {
                error("No alias env active.");
            };
            cur_env
        },
        /* Set on first use. */
        None => {
            error(&format!("${} does not exist. Rerun setup.", ENV_VAR))
        }
    }.into_string().unwrap();

    let alias_file = get_alias_file(&env)
        .into_os_string()
        .into_string()
        .unwrap();

    /* Delete alias from file if possible. */
    if delete_alias_from_file(&alias_file, alias) {
        let temp = String::from(format!("unalias {}", alias));
        add_output_line(output, &temp);
    } else {
        error("No such alias.");
    }
}

/*********/
/* Main. */
/*********/

fn main() {
    let mut output: String = String::new();
    setup(&mut output);
    
    let matches = clap_app!(alienv =>
        (version: "1.0")
        (author: "Pranav K. <pmkumar@cmu.edu>")
        (about: "Alias environment manager")
        (@subcommand new =>
            (about: "Creates new environment and switch to it")
            (@arg env_name: +required "Name of the environment to create")
        )
        (@subcommand delete =>
            (about: "Deletes existing environment")
            (@arg env_name: +required "Name of the environment to delete")
        )
        (@subcommand load =>
            (about: "Switches to existing environment")
            (@arg env_name: +required "Name of the environment to load")
        )
        (@subcommand show =>
            (about: "Displays existing environments")
        )
        (@subcommand add =>
            (about: "Adds alias to current environment")
            (@arg alias_name: +required "Name of the alias to add")
            (@arg command:    +required "Command to alias")
        )
        (@subcommand rem =>
            (about: "Removes alias from current environment")
            (@arg alias_name: +required "Name of the alias to remove")
        )
    )
    .setting(AppSettings::DisableVersion)
    .setting(AppSettings::VersionlessSubcommands)
    .get_matches_safe()
    .map_err(|e| err_clap(e))
    .expect("Invalid arguments");
    
    if let Some(matches) = matches.subcommand_matches("new") {
        new(&mut output, matches.value_of("env_name").unwrap());
        
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        delete(&mut output, matches.value_of("env_name").unwrap());
        
    } else if let Some(matches) = matches.subcommand_matches("load") {
        load(&mut output, matches.value_of("env_name").unwrap());
        
    } else if let Some(_) = matches.subcommand_matches("show") {
        show_all(&mut output);
        
    } else if let Some(matches) = matches.subcommand_matches("add") {
        let alias = matches.value_of("alias_name").unwrap();
        let command = matches.value_of("command").unwrap();
        add_alias(&mut output, alias, command);
    } else if let Some(matches) = matches.subcommand_matches("rem") {
        let alias = matches.value_of("alias_name").unwrap();
        remove_alias(&mut output, alias);
    }
    
    println!("{}", output);
}