use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} <num1> <op> <num2>“, args[0]);
        std::process::exit(1);
    }

    let num1 = args[1].parse::<f64>().ok();
    let op = &args[2];
    let num2 = args[3].parse::<f64>().ok();

    if num1.is_none() || num2.is_none() {
        println!("Invalid number input");
        std::process::exit(1);
    }

    let num1 = num1.unwrap();
    let num2 = num2.unwrap();

    let result = match op {
        "+" => num1 + num2,
        "-" => num1 - num2,
        "*" => num1 * num2,
        "/" => {
            if num2 == 0.0 {
                panic!("Division by zero");
            }
            num1 / num2
        },
        _ => {
            println!("Unknown operator {}", op);
            std::process::exit(1);
        }
    };

    println!("Result: {}", result);
}