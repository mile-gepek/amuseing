use slint_build::CompilerConfiguration;

fn main() {
    // slint_build::compile("ui/main.slint").expect("Slint build failed");
    let config = CompilerConfiguration::new().with_style("fluent-dark".to_string());
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint build failed");
}
