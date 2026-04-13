//! Quality-assurance gate — runs all checks in sequence and exits non-zero on
//! any failure. Invoke via `cargo qa`.

use std::process::{Command, ExitCode};
use std::time::Instant;

struct Step {
    label: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

const STEPS: &[Step] = &[
    Step {
        label: "Formatting",
        program: "cargo",
        args: &["fmt", "--all", "--check"],
    },
    Step {
        label: "Clippy (library)",
        program: "cargo",
        args: &[
            "clippy",
            "--workspace",
            "--lib",
            "--",
            "-D",
            "clippy::unwrap_used",
            "-D",
            "clippy::todo",
        ],
    },
    Step {
        label: "Tests",
        program: "cargo",
        args: &["test", "--workspace"],
    },
    Step {
        label: "Ignored tests",
        program: "cargo",
        args: &["test", "--workspace", "--", "--ignored"],
    },
    Step {
        label: "Validation",
        program: "cargo",
        args: &["val"],
    },
];

fn main() -> ExitCode {
    let total = Instant::now();
    let mut failures: Vec<&str> = Vec::new();

    for step in STEPS {
        eprintln!("\n==> {}", step.label);
        let start = Instant::now();

        let status =
            Command::new(step.program).args(step.args).status();

        let elapsed = start.elapsed();

        match status {
            Ok(s) if s.success() => {
                eprintln!("    PASS  ({:.1}s)", elapsed.as_secs_f64());
            }
            Ok(_) => {
                eprintln!("    FAIL  ({:.1}s)", elapsed.as_secs_f64());
                failures.push(step.label);
            }
            Err(e) => {
                eprintln!(
                    "    ERROR could not run `{}`: {e}",
                    step.program
                );
                failures.push(step.label);
            }
        }
    }

    eprintln!("\n{}", "=".repeat(40));
    eprintln!(
        "QA finished in {:.1}s — {}/{} passed",
        total.elapsed().as_secs_f64(),
        STEPS.len() - failures.len(),
        STEPS.len(),
    );

    if failures.is_empty() {
        eprintln!("All checks passed.");
        ExitCode::SUCCESS
    } else {
        eprintln!("Failures:");
        for f in &failures {
            eprintln!("  - {f}");
        }
        ExitCode::FAILURE
    }
}
