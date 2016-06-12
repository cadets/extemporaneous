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

use super::*;


/// Trait for things that have a GraphViz dot representation
pub trait ToDot {
    /// Produce a GraphViz dot representation
    fn dot(&self) -> String;
}


impl ToDot for Automaton {
    fn dot(&self) -> String {
        let states = self.states
            .iter()
            .enumerate()
            .map(|(id,s)|
                 format!["s{} {};", id, attrs(&vec![
                        ("label", s.label(id, &self.variables)),
                        ("shape", (if s.accepting { "doublecircle" } else { "circle" }).to_string()),
                    ])
                ])
            .collect::<Vec<String>>()
            .join("\n")
            ;

        let transitions = self.transitions
            .iter_all()
            .map(|(ref source, ref transitions)| {
                transitions.iter()
                    .map(|&(ref t, ref dest)| {
                        format!["s{} -> s{} [ label = \"{}\" ];", source, dest, t.dot()]
                    })
                    .collect::<Vec<String>>()
                    .join("\n")
            })
            .collect::<Vec<String>>()
            .join("\n")
            ;

        format!["digraph {{\nrankdir=LR;\n{}\n\n{}\n}}", states, transitions]
    }
}


impl ToDot for Transition {
    fn dot(&self) -> String {
        let cause = self.cause.name();
        let result = self.result.short_name();

        if result.len() == 0 {
            cause
        } else {
            format!["{} &laquo;{}&raquo;", cause, result]
        }
    }
}


/// Transform key-value pairs into a Dot attribute list
fn attrs(attrs: &[(&str,String)]) -> String {
    format!["[ {} ]",
        attrs.iter()
             .map(|&(ref k, ref v)|
                  format!["{} = {}", k, dot_value(v)])
             .collect::<Vec<String>>()
             .join(", ")
    ]
}

/// Quote a value for use as a GraphViz attribute, if required.
///
/// It's generally safe to quote values, but if the string passed in is an HTML-style label
/// (e.g., "<q<sub>0</sub>>") we shouldn't add quotes.
fn dot_value<T>(value: &T) -> String
    where T : Clone + Into<String>
{
    let s = value.clone().into();

    if s.len() > 2 && s.starts_with('<') && s.ends_with('>') {
        s
    } else {
        format!["\"{}\"", s]
    }
}
