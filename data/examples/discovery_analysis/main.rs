use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        4 => simulate(
            args[1].parse().unwrap(),
            args[2].parse().unwrap(),
            args[3].parse().unwrap(),
        ),
        3 => round(args[1].parse().unwrap(), args[2].parse().unwrap()),
        _ => {
            println!("usage: {} stations time_slots [addresses]", args[0]);
            0
        }
    };
}

fn simulate(mut stations: i32, slots: i32, mut addresses: i32) -> i32 {
    let mut i = 1;

    while stations > 0 {
        println!("-----");
        println!("Round {i}:");
        let received = round(stations, slots);
        let assigned = round(received, addresses);
        stations -= assigned;
        addresses -= assigned;
        i += 1;
    }

    i - 1
}

fn round(n: i32, m: i32) -> i32 {
    // Probability of no collision.
    let mut p_bar = 1.0;

    for i in 1..n {
        p_bar = p_bar * (1.0 - i as f64 / m as f64);
    }

    // Probability of collision.
    let p = 1.0 - p_bar;

    // Expected non collisions
    let e = n as f64 * (1.0 - 1.0 / m as f64).powi(n - 1);

    println!("For {n} of {m} Prob collision = {p:0.2} Expected successes = {e:0.1}");

    e.round() as i32
}
