use colibri_simulator::run_happy_path_json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", run_happy_path_json()?);
    Ok(())
}
