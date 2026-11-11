//! The `kilat` command: run, check and test Kilat programs.

use kilat_core::compile::Program;
use kilat_core::vm::{Status, Vm};
use kilat_core::{build, resolve_import, VERSION};
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const USAGE: &str = "\
usage:
  kilat run <file.kilat> [args...]   run a program (short form: kilat <file.kilat>)
  kilat check <file.kilat>           check types and units without running
  kilat test <file.kilat>            run the `test \"name\" { ... }` blocks
  kilat eval \"<code>\"                run a line of code and print its value (short form: -e)
  kilat version

options:
  --fuel <n>       stop the program after n steps (default: no limit)
  --mem <MB>       memory limit in megabytes (default: 2048)
  --allow-read     let the program read files
  --allow-write    let the program write files
  --               everything after this goes to the program (see args())
";

/// How much fuel one `run` call may use before control comes back here.
const SLICE: u64 = 50_000_000;

struct Options {
    fuel: u64,
    memory: usize,
    allow_read: bool,
    allow_write: bool,
    program_args: Vec<String>,
    started: Instant,
}

/// Where the program's output goes.
enum Sink<'a> {
    Stdout,
    Discard,
    Capture(&'a mut String),
}

fn seed() -> u64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let here = &nanos as *const u64 as u64;
    nanos ^ here.rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15
}

fn write_out(text: &str) {
    if text.is_empty() {
        return;
    }
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if lock.write_all(text.as_bytes()).and_then(|_| lock.flush()).is_err() {
        // The reader went away (for example `kilat run x | more` was closed).
        std::process::exit(0);
    }
}

fn read_text(path: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text)),
        Err(e) => Err(format!("cannot read `{}`: {}", path, e)),
    }
}

/// Answer one call through the host gate.
fn answer(vm: &mut Vm, options: &Options) {
    let name = vm.host_name.clone();
    let payload = vm.host_payload.clone();
    let result: Result<String, String> = match name.as_str() {
        "read_file" => {
            if options.allow_read {
                read_text(&payload)
            } else {
                Err("reading files is not allowed (run with --allow-read)".to_string())
            }
        }
        "write_file" => {
            let (path, text) = payload.split_once('\n').unwrap_or((payload.as_str(), ""));
            if !options.allow_write {
                Err("writing files is not allowed (run with --allow-write)".to_string())
            } else {
                match std::fs::write(path, text) {
                    Ok(()) => Ok(String::new()),
                    Err(e) => Err(format!("cannot write `{}`: {}", path, e)),
                }
            }
        }
        "input" => {
            write_out(&payload);
            let mut line = String::new();
            match std::io::stdin().lock().read_line(&mut line) {
                Ok(_) => Ok(line.trim_end_matches(['\r', '\n']).to_string()),
                Err(e) => Err(format!("cannot read the input: {}", e)),
            }
        }
        "clock" => Ok(format!("{}", options.started.elapsed().as_secs_f64())),
        "time" => {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            Ok(format!("{}", now))
        }
        "args" => Ok(options.program_args.join("\n")),
        "http_get" | "ask_ai" => Err("not available in the kilat CLI".to_string()),
        other => Err(format!("unknown host function `{}`", other)),
    };
    match result {
        Ok(text) => vm.resume(true, &text),
        Err(text) => vm.resume(false, &text),
    }
}

/// Run the VM until it finishes. Returns false when it stopped with an error (see `vm.error`).
fn drive(vm: &mut Vm, options: &Options, mut sink: Sink) -> bool {
    loop {
        let status = vm.run(SLICE);
        match &mut sink {
            Sink::Stdout => write_out(&vm.core.out),
            Sink::Discard => {}
            Sink::Capture(buffer) => buffer.push_str(&vm.core.out),
        }
        vm.core.out.clear();
        match status {
            Status::Done => return true,
            Status::More => {}
            Status::Host => answer(vm, options),
            Status::Error => return false,
        }
    }
}

fn compile(name: &str, source: &str, print_last: bool) -> Result<Program, String> {
    let mut loader = |path: &str, importer: &str| -> Result<(String, String), String> {
        let resolved = resolve_import(importer, path);
        match std::fs::read_to_string(&resolved) {
            Ok(text) => Ok((resolved, text)),
            Err(e) => Err(format!("cannot read `{}`: {}", resolved, e)),
        }
    };
    build(name, source, &mut loader, print_last)
}

fn run_program(program: Program, options: &Options) -> i32 {
    let mut vm = Vm::new(Rc::new(program), seed(), options.memory, options.fuel);
    let code = if drive(&mut vm, options, Sink::Stdout) {
        0
    } else {
        eprintln!("{}", vm.error);
        1
    };
    // The process ends right after this, so leave the program's memory to the system
    // instead of freeing every object one by one.
    std::mem::forget(vm);
    code
}

fn run_tests(path: &str, program: Program, options: &Options) -> i32 {
    let program = Rc::new(program);
    if program.tests.is_empty() {
        println!("no tests found in {}", path);
        return 0;
    }
    let (mut passed, mut failed) = (0, 0);
    for (name, test_fn) in &program.tests {
        // Every test starts from a fresh program state.
        let mut vm = Vm::new(program.clone(), seed(), options.memory, options.fuel);
        if !drive(&mut vm, options, Sink::Discard) {
            eprintln!("{}", vm.error);
            eprintln!("the top-level code failed, so no tests were run");
            return 1;
        }
        vm.start(*test_fn);
        let mut output = String::new();
        if drive(&mut vm, options, Sink::Capture(&mut output)) {
            println!("ok    {}", name);
            passed += 1;
        } else {
            println!("FAIL  {}", name);
            for line in output.lines().chain(vm.error.lines()) {
                println!("      {}", line);
            }
            failed += 1;
        }
    }
    println!("\n{} passed, {} failed", passed, failed);
    if failed == 0 {
        0
    } else {
        1
    }
}

fn number_after(flag: &str, value: Option<&String>) -> Result<u64, String> {
    let text = value.ok_or_else(|| format!("`{}` needs a number after it", flag))?;
    text.replace('_', "").parse::<u64>().map_err(|_| format!("`{}` needs a whole number, found `{}`", flag, text))
}

fn real_main() -> i32 {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut options = Options {
        fuel: u64::MAX,
        memory: 2048usize.saturating_mul(1 << 20),
        allow_read: false,
        allow_write: false,
        program_args: Vec::new(),
        started: Instant::now(),
    };
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let arg = raw[i].as_str();
        let parsed: Result<(), String> = match arg {
            "--" => {
                options.program_args.extend(raw[i + 1..].iter().cloned());
                break;
            }
            "--allow-read" => {
                options.allow_read = true;
                Ok(())
            }
            "--allow-write" => {
                options.allow_write = true;
                Ok(())
            }
            "--fuel" => {
                i += 1;
                number_after(arg, raw.get(i)).map(|n| options.fuel = n)
            }
            "--mem" => {
                i += 1;
                number_after(arg, raw.get(i)).map(|mb| options.memory = (mb as usize).saturating_mul(1 << 20))
            }
            _ => {
                words.push(raw[i].clone());
                Ok(())
            }
        };
        if let Err(message) = parsed {
            eprintln!("error: {}", message);
            return 2;
        }
        i += 1;
    }

    let command = words.first().map(String::as_str).unwrap_or("help");
    match command {
        "help" | "--help" | "-h" => {
            println!("kilat {} - a small, safe, unit-aware language\n\n{}", VERSION, USAGE);
            return 0;
        }
        "version" | "--version" | "-V" => {
            println!("kilat {}", VERSION);
            return 0;
        }
        _ => {}
    }
    // `kilat file.kilat` is short for `kilat run file.kilat`.
    let (command, rest) = match command {
        "run" | "check" | "test" | "eval" | "-e" => (command, &words[1..]),
        _ => ("run", &words[..]),
    };
    let target = match rest.first() {
        Some(target) => target.clone(),
        None => {
            eprintln!("error: `kilat {}` needs {}\n\n{}", command, if command == "eval" || command == "-e" { "some code" } else { "a file" }, USAGE);
            return 2;
        }
    };
    let mut program_args: Vec<String> = rest[1..].to_vec();
    program_args.append(&mut options.program_args);
    options.program_args = program_args;

    let is_eval = command == "eval" || command == "-e";
    let (name, source) = if is_eval {
        ("<eval>".to_string(), target.clone())
    } else {
        match read_text(&target) {
            Ok(text) => (target.clone(), text),
            Err(message) => {
                eprintln!("error: {}", message);
                return 1;
            }
        }
    };
    let program = match compile(&name, &source, is_eval) {
        Ok(program) => program,
        Err(text) => {
            eprintln!("{}", text);
            return 1;
        }
    };
    match command {
        "check" => {
            println!("ok: {} has no errors", target);
            0
        }
        "test" => run_tests(&target, program, &options),
        _ => run_program(program, &options),
    }
}

fn main() {
    // The compiler recurses over nested code, so give it room.
    let worker = std::thread::Builder::new().stack_size(64 << 20).spawn(real_main);
    let code = match worker.map(|handle| handle.join()) {
        Ok(Ok(code)) => code,
        _ => 101,
    };
    std::process::exit(code);
}
