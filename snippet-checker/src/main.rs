use colored::*;
use regex::Regex; // For matching Rust code blocks in markdown files
use similar::{ChangeTag, TextDiff}; // For calculating and displaying differences

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir; // For recursively iterating through directories

// The book source directory is a sibling from current one
const MDBOOK_DIR: &str = "../src";

fn bold_red(str: &str) -> ColoredString {
    str.bold().red()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    control::set_override(true); // Force colored output for CI environment
    let mut final_diff = false;

    // Walk through all files in the mdBook directory recursively
    for entry in WalkDir::new(MDBOOK_DIR)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
    {
        // Check if the current file has the `.md` extension
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let md_path = entry.path();
        let md_content = fs::read_to_string(md_path)?;

        print!("{} ", md_path.strip_prefix(MDBOOK_DIR).unwrap().display());
        std::io::stdout().flush().unwrap();

        match get_md_snippets_diff(md_content)? {
            Some(true) => final_diff = true, // Diff found
            Some(false) => println!("... {}", "ok".green()),
            None => println!("... {}", "no snippets".yellow()),
        }
    }

    if final_diff {
        println!("\nFinal status: {}", "DIFF FOUND".bold().red());
        Err("Diff found".into())
    } else {
        println!("\nFinal status: {}", "OK".green());
        Ok(())
    }
}

// Returns `None` if there was no snippet, `Some(false)` if there was a snippet with no difference
// with the floresta code, or `Some(true)` if there was a difference
fn get_md_snippets_diff(md_file: String) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let rust_code_regex = Regex::new(r"(?s)```rust\n# // Path: (.*?)\n(.*?)\n```")?;

    // Track if there is any difference between the code and the book snippets
    let mut diff = None;

    // Strip '> ' prefix from content, as some snippets are inside blockquotes
    let md_file = md_file
        .lines()
        .map(|line| line.strip_prefix("> ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");

    for (i, caps) in rust_code_regex.captures_iter(&md_file).enumerate() {
        let path = caps.get(1).unwrap().as_str();
        let snippet = caps.get(2).unwrap().as_str();
        if i == 0 {
            diff = Some(false);
        }

        // Check that the path retrieved from the mdbook snippet exists
        let code_path = validate_file_path(path).unwrap_or_else(|| {
            panic!(
                "\n{} - {}\n",
                bold_red(&format!(
                    "Warning: File path read from snippet {i} does not exist"
                )),
                path,
            )
        });

        let code_content = fs::read_to_string(&code_path)?;
        let cleaned_snippet = strip_comments(snippet);
        assert!(
            remove_identation(&cleaned_snippet).is_none(),
            "Snippets are expected to not have identation in all the lines",
        );

        // Get the matching code content, and the line where it is found
        let (block_start_line, mut block) = extract_clean_block(&code_content, &cleaned_snippet)
            .unwrap_or_else(|| {
                panic!(
                    "\n{} in {}\n",
                    bold_red(&format!(
                        "Warning: Could not find matching block for snippet {i}"
                    )),
                    path,
                )
            });

        if cleaned_snippet != block {
            if let Some(no_ident_block) = remove_identation(&block) {
                if cleaned_snippet == no_ident_block {
                    // Continue with the next snippet, this one actually matches
                    continue;
                } else {
                    // Since the block does have identation in all the lines we need to use the
                    // `no_ident_block` to properly display the diff (as snippet is also trimmed)
                    block = no_ident_block;
                }
            }

            diff = Some(true);
            print!("... {}\n\n", "DIFF".bold().red());
            println!("Snippet index: {}", i.to_string().bold().yellow());
            println!(
                "Code: {}:{}",
                path.bold().yellow(),
                block_start_line.to_string().bold()
            );

            println!();
            print_diff(&cleaned_snippet, &block);
        }
    }

    Ok(diff)
}

fn remove_identation(block: &str) -> Option<String> {
    let mut no_ident = String::new();
    let to_remove = " ".repeat(4); // Identation is 4 spaces

    for (i, line) in block.lines().enumerate() {
        if let Some(trimmed) = line.strip_prefix(&to_remove) {
            if i != 0 {
                // End previous line
                no_ident.push('\n');
            }
            no_ident.push_str(trimmed);
        } else {
            // The block doesn't have identation in all the lines
            return None;
        }
    }
    Some(no_ident)
}

// Function to validate the extracted file path and ensure it exists
fn validate_file_path(snippet_path: &str) -> Option<PathBuf> {
    let code_dir = env::var("CODE_DIR").expect("CODE_DIR environment variable is not set");

    let file_path = format!("{}/crates/{}", code_dir, snippet_path);
    let path = Path::new(&file_path);

    if path.try_exists().is_ok() && path.is_file() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

// Function to get the whole snippet, including ignored lines and excluding comments and empty lines
fn strip_comments(code: &str) -> String {
    code.lines()
        .map(|line| {
            let trimmed = line.trim_start();

            // Remove any leading `#` when not an #[attribute]
            if trimmed.starts_with('#') && !trimmed.starts_with("#[") {
                let hash_index = line.find('#').unwrap();

                let before = &line[..hash_index];
                let after = &line[hash_index + 1..].trim_start(); // Remove spaces after `#`
                format!("{}{}", before, after)
            } else {
                line.to_string()
            }
        })
        .filter(|line| {
            // Keep lines that are not comments and are not empty
            !line.trim_start().starts_with("//") && !line.trim().is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Function to print the differences between the documentation snippet and the actual code
fn print_diff(doc_code: &str, real_code: &str) {
    let diff = TextDiff::from_lines(doc_code, real_code); // Generate the diff
    for change in diff.iter_all_changes() {
        // Iterate through each change and format it visually
        match change.tag() {
            ChangeTag::Delete => {
                print!("{}", format!("- {}", change).red()); // Deleted lines in red
            }
            ChangeTag::Insert => {
                print!("{}", format!("+ {}", change).green()); // Added lines in green
            }
            ChangeTag::Equal => {
                print!("{}", format!("  {}", change).white()); // Unchanged lines in white
            }
        }
    }
    println!(); // Add a blank line after printing the diff
}

// Extract the block of code from the file based on the snippet
fn extract_clean_block(file_content: &str, snippet: &str) -> Option<(usize, String)> {
    let snippet_lines = snippet.lines().count();
    let first_line = snippet.lines().find(|line| !line.trim().is_empty())?; // Get the first meaningful line
    let mut block = String::new();
    let mut inside_block = false;

    let mut block_start_line = 0;
    let mut block_lines = 0;

    for (i, line) in file_content.lines().enumerate() {
        if !inside_block && line.trim() == first_line.trim() {
            inside_block = true; // Start capturing the block
            block_start_line = i + 1; // The code lines start at number 1
        }

        if inside_block {
            // Only take the lines that are not comments nor empty
            if !line.trim_start().starts_with("//") && !line.trim().is_empty() {
                if block_lines != 0 {
                    // End previous line
                    block.push('\n');
                }
                block.push_str(line);
                block_lines += 1;
            }
            assert_eq!(block.lines().count(), block_lines);

            // End capturing if we have captured all lines
            if block_lines == snippet_lines {
                break;
            }
        }
    }

    // Return the block if anything is captured
    if inside_block {
        Some((block_start_line, block))
    } else {
        None
    }
}
