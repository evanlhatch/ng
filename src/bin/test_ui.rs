//! Simple test program for UI styling

use ng::ui_style::{Colors, Symbols, Print};

fn main() {
    println!("Testing UI styling...");
    
    // Test colors
    println!("Success: {}", Colors::success("This is a success message"));
    println!("Error: {}", Colors::error("This is an error message"));
    println!("Warning: {}", Colors::warning("This is a warning message"));
    println!("Info: {}", Colors::info("This is an info message"));
    println!("Prompt: {}", Colors::prompt("This is a prompt message"));
    println!("Code: {}", Colors::code("This is a code message"));
    println!("Emphasis: {}", Colors::emphasis("This is an emphasized message"));
    
    // Test symbols
    println!("Success symbol: {}", Symbols::success());
    println!("Error symbol: {}", Symbols::error());
    println!("Warning symbol: {}", Symbols::warning());
    println!("Info symbol: {}", Symbols::info());
    println!("Progress symbol: {}", Symbols::progress());
    println!("Cleanup symbol: {}", Symbols::cleanup());
    println!("Prompt symbol: {}", Symbols::prompt());
    
    // Test print functions
    Print::success("This is a success message");
    Print::error("This is an error message");
    Print::warning("This is a warning message");
    Print::info("This is an info message");
    Print::prompt("This is a prompt message");
    Print::section("This is a section header");
    
    println!("UI styling test completed!");
}