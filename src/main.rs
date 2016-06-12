/*
 * Copyright 2016 Jonathan Anderson
 *
 * This software was developed by BAE Systems, the University of Cambridge
 * Computer Laboratory, and Memorial University under DARPA/AFRL contract
 * FA8650-15-C-7558 ("CADETS"), as part of the DARPA Transparent Computing
 * (TC) research program.
 *
 * Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
 * http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
 * http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */

extern crate docopt;
extern crate extemp;
extern crate rustc_serialize;

use docopt::Docopt;
use extemp::*;
use extemp::determinism::IntoDFA;
use extemp::dot::ToDot;
use std::fs::File;
use std::io;
use std::io::Write;


// TODO: use docopt_macros once rust-lang/rust#28089 is resolved
const USAGE: &'static str = "
Usage: extemp [options]
       extemp (--help | --version)

Options:
    -h, --help             Show this message
    -f, --format=<format>  Output format [default: dot]
    -o, --output=<file>    Output file [default: graph.dot]
    -v, --version          Show the version of rshark
";

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


#[derive(RustcDecodable)]
struct Args {
    flag_format: String,
    flag_output: String,
    flag_version: bool,
}


fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(std::env::args()).decode())
        .unwrap_or_else(|e| e.exit())
        ;

    println!["extemp v{}", VERSION.unwrap_or("<unknown>")];

    if args.flag_version {
        return;
    }

    let mut a = Automaton::new("NFA",
        "Based on example from https://en.wikipedia.org/wiki/Powerset_construction",
        vec!["x".to_string()]);

    let s1 = a.add_state(1, false);
    let s2 = a.add_state(1, false);
    let s3 = a.add_state(1, true);
    let s4 = a.add_state(1, true);

    a.add_transition(0, s1, Transition::new(Event::named("0"), Action::Fork));
    a.add_transition(s1, s2, Transition::new(Event::named("0"), Action::Update));
    a.add_transition(s1, s3, Transition::epsilon());
    a.add_transition(s2, s2, Transition::new(Event::named("1"), Action::Update));
    a.add_transition(s2, s4, Transition::new(Event::named("1"), Action::Update));
    a.add_transition(s3, s2, Transition::epsilon());
    a.add_transition(s3, s4, Transition::new(Event::named("0"), Action::Update));
    a.add_transition(s4, s3, Transition::new(Event::named("0"), Action::Update));

    File::create("nfa.dot")
         .and_then(|mut f| f.write(a.dot().as_bytes()))
         .unwrap()
         ;

    a.dfa().and_then(|dfa| {
        match args.flag_format.as_str() {
            "dot" => {
                File::create(&args.flag_output)
                    .and_then(|mut f| f.write(dfa.dot().as_bytes()))
                    .map(|_| ())
            },

            format => write![io::stderr(), "Unknown file format: {}", format],
        }
        .map_err(Error::IO)
    }).unwrap();
}
