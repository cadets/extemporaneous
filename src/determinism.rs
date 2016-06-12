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

extern crate bit_set;
extern crate multimap;

use super::*;

use multimap::MultiMap;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied,Vacant};
use std::sync::mpsc;


/// Trait for converting an NFA into a DFA
pub trait IntoDFA {
    ///
    /// Convert the automaton from an NFA into a DFA.
    ///
    /// # Errors
    ///
    /// May return an `Error::Automaton` if there is a problem with the automaton that
    /// prevents it from being converted into a DFA.
    ///
    fn dfa(self) -> Result<Automaton>;
}


/// Something that can hold NFA states.
trait HasStates {
    fn add(&mut self, s: StateID) -> &mut NFAStates;
    fn plus(self, s: StateID) -> NFAStates;
    fn union(mut self, other: NFAStates) -> NFAStates;
}


/// A set of NFA states that will be represented by a DFA state.
type NFAStates = bit_set::BitSet;


/// A structure that keeps track of NFA-DFA state mappings
struct StateMapper {
    /// The DFA states, built as needed
    dfa_states: Vec<State>,

    /// Mappings from sets of NFA states to DFA states as they are built
    map: HashMap<NFAStates,StateID>,

    /// A channel for sending notifications of new DFA states
    state_notifier: mpsc::Sender<StateMapping>,
}


/// A mapping from a set of NFA states to a DFA state.
type StateMapping = (NFAStates, StateID);



impl IntoDFA for Automaton {
    ///
    /// Apply the Rabin-Scott powerset construction with ε-transitions to convert an NFA
    /// into a DFA.
    ///
    fn dfa(self) -> Result<Automaton> {
        let all_nfa_states = (0..self.states.len())
            .map(|s| s as StateID)
            .collect::<Vec<_>>()
            ;

        // Create a state mapper and a channel to receive notifications on
        // ("here's a new set of NFA states that you need to look at the transitions of").
        let (mut state_map, state_rx) = StateMapper::new();
        let mut dfa_transitions = MultiMap::<StateID, (Transition, StateID)>::new();

        // Start by calculating DFA states that are ε-closures of NFA states.
        // This will cause some NFAStates to be sent to state_rx.
        try![state_map.add_epsilon_closures(&all_nfa_states, &self)];

        // Iterate over NFA node sets, examining transitions out from its ε-closure
        // (which may discover more NFA node sets that we need to examine along the way).
        // Stop when there are no more NFA node sets left to look at.
        while let Ok((nfa_source,dfa_source)) = state_rx.try_recv() {
            let mut state_transitions = HashMap::<&Transition, NFAStates>::new();

            // Find the ε-closure of each transition's destination and union it into the
            // set of NFA nodes that it leads to.
            for (s, transitions) in self.transitions.iter_all() {
                if !nfa_source.contains(*s as usize) {
                    continue
                }

                for &(ref transition, destination) in transitions {
                    // Ignore epsilon transitions: handled by ε-closures
                    if let Event::Epsilon = transition.cause {
                        continue
                    }

                    let dest_closure = epsilon_closure(destination, &self.transitions);

                    match state_transitions.entry(transition) {
                        Occupied(ref mut o) => { o.get_mut().union_with(&dest_closure); },
                        Vacant(v) => { v.insert(dest_closure); },
                    };
                }
            }

            // Turn this set of NFA nodes into a DFA node and create the DFA transition.
            for (transition, destination) in state_transitions {
                //let dest = try![get_dfa(destination)];
                let dest = try![state_map.dfa_state(destination, &self)];
                dfa_transitions.insert(dfa_source, (transition.clone(), dest));
            }
        }

        // All done! Create the Automaton struct for the DFA, consuming the
        // remaining fields of the original NFA.
        Ok(Automaton {
            name: self.name,
            description: self.description,
            states: state_map.dfa_states,
            transitions: dfa_transitions,
            variables: self.variables,
        })
    }
}


impl HasStates for NFAStates {
    fn add(&mut self, s: StateID) -> &mut NFAStates {
        self.insert(s as usize);
        self
    }

    fn plus(mut self, s: StateID) -> NFAStates {
        self.add(s);
        self
    }

    fn union(mut self, other: NFAStates) -> NFAStates {
        self.union_with(&other);
        self
    }
}


impl StateMapper {
    ///
    /// Create a new StateMapper as well as a channel for receiving "new state" notifications on.
    ///
    fn new() -> (StateMapper, mpsc::Receiver<StateMapping>) {
        let (tx, rx): (mpsc::Sender<StateMapping>, mpsc::Receiver<StateMapping>) = mpsc::channel();

        let map = StateMapper {
            dfa_states: Vec::new(),
            map: HashMap::new(),
            state_notifier: tx,
        };

        (map, rx)
    }

    ///
    /// Find or create a DFA state that corresponds to one or more NFA states.
    ///
    fn dfa_state(&mut self, nfa_states: NFAStates, a: &Automaton) -> Result<StateID> {
        if nfa_states.is_empty() {
            return Err(Error::Automaton("empty set of NFA states".to_string()))
        }

        // TODO: use entry.key() once Rust 1.10 drops:
        // see https://github.com/rust-lang/rust/issues/32281
        let tmp_states = nfa_states.clone();

        match self.map.entry(nfa_states) {
            // If we already have this state, just return it.
            Occupied(entry) => Ok(*entry.get()),

            // Otherwise, create a new DFA state with the same mask as the NFA states
            // (which should be the same!) and the union of their acceptance flags.
            Vacant(entry) => {
                let mut mask:Mask = 0;
                let mut accepting = false;

                for i in tmp_states.iter() {
                    let ref s = a.states[i];

                    if mask == 0 {
                        mask = s.variable_mask;
                    } else if mask != s.variable_mask {
                        return Err(
                            Error::Automaton(
                                format!["inconsistent masks in {:?}: {} has {}, not {}",
                                        tmp_states, i, s.variable_mask, mask])
                        )
                    }

                    accepting |= s.accepting;
                }

                let id = self.dfa_states.len() as StateID;
                let name = Some(format!["{:?}", tmp_states]);

                self.dfa_states.push(State::new(mask, accepting, name));
                try![self.state_notifier
                         .send((tmp_states, id))
                         .map_err(|e| Error::Internal(format!["channel error: {}", e]))];

                entry.insert(id);
                Ok(id)
            },
        }
    }


    ///
    /// Add DFA states for all NFA states' epsilon closures.
    ///
    fn add_epsilon_closures(&mut self, states: &[StateID], a: &Automaton) -> Result<()> {
        // Start by calculating DFA states that are ε-closures of NFA states.
        let mut done = NFAStates::new();
        for &s in states {
            // Skip states that have already been considered as part of another state's ε-closure.
            if done.contains(s as usize) {
                continue
            }

            let e_closure = epsilon_closure(s, &a.transitions);
            done.union_with(&e_closure);
            try![self.dfa_state(e_closure, a)];
        }

        Ok(())
    }
}


///
/// Calculate all of the NFA states that can be reached from a given state by ε-transitions.
///
fn epsilon_closure(source: StateID, transitions: &TransitionMap) -> NFAStates {
    let empty = Vec::<(Transition,StateID)>::new();

    // Get vector of transitions (or empty vector):
    transitions
        .get_vec(&source)
        .unwrap_or(&empty)

        // Find destinations of ε-transitions (ignore non-ε transitions):
        .into_iter()
        .map(|x| match x {
            &(Transition { cause:Event::Epsilon, .. }, ref dest) => Some(dest),
            _ => None,
        })
        .filter(Option::is_some)
        .map(Option::unwrap)

        // Recursively find the ε-closure of each destination node and merge them together:
        .map(|&source| epsilon_closure(source, transitions))
        .fold(NFAStates::new(), HasStates::union)

        // Add in the source node (i.e., the ε-closure of state 2 includes state 2):
        .plus(source)
}
