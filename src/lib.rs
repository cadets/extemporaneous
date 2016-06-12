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

extern crate multimap;

use multimap::MultiMap;


/// An integer type that controls how many bits are in a variable binding mask,
/// therefore how many variable a particular automaton instance can bind to.
pub type Mask = u32;

/// An integer type that controls how many states an automaton can have.
pub type StateID = u16;

/// The type of value that can be bound to variables.
pub type Value = u64;

/// A type for the transitions that can be taken out from a state and their destinations.
pub type TransitionMap = MultiMap<StateID, (Transition, StateID)>;


/// An error that can be encountered in automaton processing.
#[derive(Debug)]
pub enum Error {
    Automaton(String),
    IO(std::io::Error),
    Internal(String),
}

/// The result of calling a potentially-failing function.
pub type Result<T,E=Error> = std::result::Result<T,E>;


///
/// Actions that may be required when we take a transition.
///
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub enum Action {
    Cleanup,
    Fork,
    Join,
    Update,
}


///
/// A static description of a temporal automaton.
///
pub struct Automaton {
    /// A unique name, hopefully human-readable
    name: String,

    /// Original source description of the automaton
    description: String,

    /// The states that an instance of this automaton can exist in
    states: Vec<State>,

    /// Transitions that will be taken in response to events
    transitions: TransitionMap,

    /// Variables that instances of this automaton can bind to values
    variables: Vec<String>,
}

#[repr(C)]
/// Opaque C structure that wraps a Rust Automaton value.
pub struct extemp_automaton(Automaton);


///
/// Events that can cause a transition to occur.
///
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Event {
    Epsilon,
    Named(String),
}


///
/// A single automaton state, which is supposed to be bound to some number of
/// variables and which may or may not be an accepting state.
///
#[derive(Clone, Debug)]
#[repr(C)]
pub struct State {
    /// A state's mask indicates which of the automaton instance's variables
    /// should be set in this state. For example, if an automaton binds to
    /// four variables and, in state 1, variables 0 and 2 should be known,
    /// the mask will be 0x05.
    variable_mask: Mask,

    /// This is an accepting state: a notification will be created and
    /// the automaton instance will be de-allocated.
    accepting: bool,

    name: Option<String>,
}


///
/// A single allowable transition in a temporal automaton.
///
/// A transition goes from one state to another, and can imply actions to be
/// taken when it occurs (e.g., clean up all automata).
///
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct Transition {
    /// The event that causes this transition
    cause: Event,

    /// The action to take on/after that transition
    result: Action,
}


impl Action {
    pub fn name(&self) -> String {
        format!["{:?}", self]
    }

    /// Provide a short name suitable for presenting to the user (e.g., Update -> "").
    pub fn short_name(&self) -> String {
        match self {
            &Action::Update => "".to_string(),
            _ => self.name().to_lowercase(),
        }
    }
}


impl Automaton {
    pub fn new<A,B>(name: A, description: B, variables: Vec<String>) -> Automaton
        where A : Into<String>, B : Into<String>
    {
        Automaton{
            name: name.into(),
            description: description.into(),
            states: vec![State::new(0, false, None)],
            transitions: TransitionMap::new(),
            variables: variables,
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> &str { &self.description }

    pub fn state(&self, id: StateID) -> &State { &self.states[id as usize] }

    pub fn add_state(&mut self, mask: Mask, accepting: bool) -> StateID {
        self.states.push(State::new(mask, accepting, None));
        (self.states.len() - 1) as StateID
    }

    pub fn add_transition(&mut self, source: StateID, dest: StateID, t: Transition)
            -> &mut Automaton {
        self.transitions.insert(source, (t, dest));
        self
    }
}


impl Event {
    pub fn named<Str>(name: Str) -> Event
        where Str: Into<String>
    {
        Event::Named(name.into())
    }

    pub fn name(&self) -> String {
        match self {
            &Event::Epsilon => "&#x3b5;".to_string(),
            &Event::Named(ref s) => s.clone(),
        }
    }
}


impl State {
    fn new(mask: Mask, accepting: bool, name: Option<String>) -> State {
        State {
            variable_mask: mask,
            accepting: accepting,
            name: name,
        }
    }

    /// Produce a Dot- or mostly-HTML-compatible label for this state
    fn label(&self, id: usize, var_names: &[String]) -> String {
        let name = self.name.clone().unwrap_or_else(|| format!["s<sub>{}</sub>", id]);
        format!["<{}<br/>({})>", name, self.variable_names(var_names)]
    }

    /// Describe the variables that ought to be bound by the time we reach this state,
    /// according to the state's variable mask and a list of variable names.
    fn variable_names(&self, names: &[String]) -> String {
        let mask = self.variable_mask;
        let star = "&#8727;";

        names
            .iter()
            .enumerate()
            .map(|(i,name)| if (mask & (1 << i)) != 0 { name.clone() } else { star.to_string() })
            .collect::<Vec<String>>()
            .join(",")
    }
}


impl Transition {
    pub fn new(cause: Event, result: Action) -> Transition {
        Transition { cause: cause, result: result }
    }

    pub fn epsilon() -> Transition {
        Transition { cause: Event::Epsilon, result: Action::Update }
    }
}


pub mod determinism;
pub mod dot;


#[cfg(test)]
mod tests {
    extern crate regex;

    use super::*;

    use self::regex::Regex;
    use super::determinism::IntoDFA;
    use super::dot::ToDot;


    #[test]
    fn test_nfa_creation() {
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

        let dot = a.dot();

        assert![dot.contains("s0 [ label = <s<sub>0</sub><br/>(&#8727;)>, shape = \"circle\" ];")];
        assert![dot.contains("s1 [ label = <s<sub>1</sub><br/>(x)>, shape = \"circle\" ];")];
        assert![dot.contains("s2 [ label = <s<sub>2</sub><br/>(x)>, shape = \"circle\" ];")];
        assert![dot.contains("s3 [ label = <s<sub>3</sub><br/>(x)>, shape = \"doublecircle\" ];")];
        assert![dot.contains("s4 [ label = <s<sub>4</sub><br/>(x)>, shape = \"doublecircle\" ];")];

        assert![dot.contains("s0 -> s1 [ label = \"0 &laquo;fork&raquo;\" ];")];
        assert![dot.contains("s4 -> s3 [ label = \"0\" ];")];
        assert![dot.contains("s1 -> s2 [ label = \"0\" ];")];
        assert![dot.contains("s1 -> s3 [ label = \"&#x3b5;\" ];")];
        assert![dot.contains("s2 -> s2 [ label = \"1\" ];")];
        assert![dot.contains("s2 -> s4 [ label = \"1\" ];")];
        assert![dot.contains("s3 -> s2 [ label = \"&#x3b5;\" ];")];
        assert![dot.contains("s3 -> s4 [ label = \"0\" ];")];
    }

    #[test]
    fn test_nfa_to_dfa() {
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

        a = a.dfa().unwrap();

        let dot = a.dot();

        let match_state = |nfa: &[StateID], accepting| {
            let states = nfa.iter().map(StateID::to_string).collect::<Vec<_>>().join(", ");
            let shape = if accepting { "doublecircle" } else { "circle" };
            let regex = format!["s([0-9]+) \\[ label = <\\{{{}\\}}.*>, shape = \"{}\" \\];",
                                states, shape];

            Regex::new(&regex)
                  .unwrap_or_else(|e| panic!["bad test regex: {}", e])
                  .captures(&dot)
                  .unwrap_or_else(|| panic!["no match for ({:?}, {}), a.k.a., '{}' in: {}",
                                            nfa, shape, regex, dot])
                  .at(1)
                  .unwrap()
                  .parse::<StateID>()
                  .unwrap_or_else(|e| panic!["integer parse error: {}", e])
        };

        let s0 = match_state(&vec![ 0 ], false);
        let s123 = match_state(&vec![ 1, 2, 3 ], true);
        let s23 = match_state(&vec![ 2, 3 ], true);
        let s24 = match_state(&vec![ 2, 4 ], true);
        let s4 = match_state(&vec![ 4 ], true);

        assert![dot.contains(&format!["s{} -> s{} [ label = \"0 &laquo;fork&raquo;\" ];", s0, s123])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"0\" ];", s123, s24])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"1\" ];", s123, s24])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"1\" ];", s23, s24])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"0\" ];", s24, s23])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"1\" ];", s24, s24])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"0\" ];", s23, s4])];
        assert![dot.contains(&format!["s{} -> s{} [ label = \"0\" ];", s4, s23])];
    }
}
