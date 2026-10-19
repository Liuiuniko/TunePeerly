// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tunepeerly_lib::run()
}
