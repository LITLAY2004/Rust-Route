use rust_route::cli::CliFormatter;

fn main() {
    println!("Testing RustRoute CLI...");
    CliFormatter::print_banner();
    CliFormatter::print_success("Test successful!");
}
