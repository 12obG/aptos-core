// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0
use aptos_types::transaction::analyzed_transaction::{AnalyzedTransaction, StorageLocation};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Node<'a> {
    txn: &'a AnalyzedTransaction,
    index: usize,
}

impl<'a> Node<'a> {
    pub(crate) fn new(txn: &'a AnalyzedTransaction, index: usize) -> Self {
        Node { txn, index }
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

pub struct DependencyGraph<'a> {
    adjacency_list: HashMap<Node<'a>, HashSet<Node<'a>>>,
    // The reverse adjacency list is used to quickly find the dependencies of a transaction.
    reverse_adjacency_list: HashMap<Node<'a>, HashSet<Node<'a>>>,
}

impl<'a> DependencyGraph<'a> {
    pub fn new() -> Self {
        DependencyGraph {
            adjacency_list: HashMap::new(),
            reverse_adjacency_list: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub fn get_adjacency_list(&self) -> &HashMap<Node<'a>, HashSet<Node<'a>>> {
        &self.adjacency_list
    }

    #[cfg(test)]

    pub fn get_reverse_adjacency_list(&self) -> &HashMap<Node<'a>, HashSet<Node<'a>>> {
        &self.reverse_adjacency_list
    }

    #[cfg(test)]
    pub fn size(&self) -> usize {
        self.adjacency_list.len()
    }

    pub fn add_dependency(&mut self, source: Node<'a>, destination: Node<'a>) {
        // Get or create the dependency set for the target transaction
        let dependencies = self
            .adjacency_list
            .entry(source)
            .or_insert_with(HashSet::new);

        let reverse_dependencies = self
            .reverse_adjacency_list
            .entry(destination)
            .or_insert_with(HashSet::new);

        // Add the source transaction to the dependency set
        dependencies.insert(destination);
        reverse_dependencies.insert(source);
    }

    pub fn get_dependent_nodes(&self, node: Node<'a>) -> Option<&'a HashSet<Node>> {
        self.reverse_adjacency_list.get(&node)
    }

    pub fn create_dependency_graph(
        analyzed_transactions: &[AnalyzedTransaction],
    ) -> DependencyGraph {
        let mut dependency_graph = DependencyGraph::new();

        let read_hint_index = Self::build_hint_index(analyzed_transactions, |txn| txn.read_hints());

        // build an index of the transactions to their indices
        let mut txn_index = HashMap::new();
        for (index, txn) in analyzed_transactions.iter().enumerate() {
            txn_index.insert(txn, index);
        }

        // Iterate through the analyzed transactions
        for (index, analyzed_txn) in analyzed_transactions.iter().enumerate() {
            // Initialize the adjecency list for the current transaction
            dependency_graph
                .adjacency_list
                .entry(Node::new(analyzed_txn, index))
                .or_insert_with(HashSet::new);
            // Initialize the reverse adjecency list for the current transaction
            dependency_graph
                .reverse_adjacency_list
                .entry(Node::new(analyzed_txn, index))
                .or_insert_with(HashSet::new);

            // Iterate through the write hints of the current transaction
            for write_hint in analyzed_txn.write_hints() {
                if let Some(transactions) = read_hint_index.get(write_hint) {
                    // Iterate through the transactions that read from the current write hint
                    for &dependent_txn in transactions {
                        if dependent_txn != analyzed_txn {
                            // Add the dependent transaction to the dependencies
                            dependency_graph.add_dependency(
                                Node::new(dependent_txn, *txn_index.get(dependent_txn).unwrap()),
                                Node::new(analyzed_txn, index),
                            );
                        }
                    }
                }
            }
        }

        dependency_graph
    }

    fn build_hint_index<F>(
        analyzed_transactions: &[AnalyzedTransaction],
        hint_selector: F,
    ) -> HashMap<&StorageLocation, HashSet<&AnalyzedTransaction>>
    where
        F: Fn(&AnalyzedTransaction) -> &[StorageLocation],
    {
        let mut index: HashMap<&StorageLocation, HashSet<&AnalyzedTransaction>> = HashMap::new();

        // Iterate through the analyzed transactions
        for analyzed_txn in analyzed_transactions {
            // Get the hints using the provided closure
            let hints = hint_selector(analyzed_txn);

            // Iterate through the hints
            for hint in hints {
                // Get or create the set of transactions associated with this hint
                let transactions = index.entry(hint).or_insert_with(HashSet::new);

                // Add the current transaction to the set
                transactions.insert(analyzed_txn);
            }
        }

        index
    }
}

#[cfg(test)]
mod tests {
    use crate::sharded_block_executor::{
        test_utils::{
            create_no_dependency_transaction, create_signed_p2p_transaction, generate_test_account,
        },
        transaction_dependency_graph::{DependencyGraph, Node},
    };
    use std::collections::HashSet;

    #[test]
    fn test_single_sender_txns() {
        let sender = generate_test_account();
        let mut receivers = Vec::new();
        let num_txns = 10;
        for _ in 0..num_txns {
            receivers.push(generate_test_account());
        }
        let transactions = create_signed_p2p_transaction(sender, receivers);
        let dependency_graph = DependencyGraph::create_dependency_graph(&transactions);
        assert_eq!(dependency_graph.size(), num_txns);
        let adjacency_list = dependency_graph.get_adjacency_list();
        let reverse_adjacency_list = dependency_graph.get_reverse_adjacency_list();
        assert_eq!(adjacency_list.len(), num_txns);
        assert_eq!(reverse_adjacency_list.len(), num_txns);
        fn assert_dependencies<'a, I>(dependencies: I, num_txns: usize)
        where
            I: Iterator<Item = (&'a Node<'a>, &'a HashSet<Node<'a>>)>,
        {
            for (node, dependencies) in dependencies {
                assert_eq!(dependencies.len(), num_txns - 1);
                let mut expected_indices: HashSet<usize> = (0..=num_txns - 1).collect();
                expected_indices.remove(&node.index());
                for dependency in dependencies {
                    expected_indices.remove(&dependency.index());
                }
                assert_eq!(expected_indices.len(), 0);
            }
        }
        assert_dependencies(adjacency_list.iter(), num_txns);
        assert_dependencies(reverse_adjacency_list.iter(), num_txns);
    }

    #[test]
    fn test_non_conflicting_txns() {
        let num_senders = 10;
        let num_receivers = 10;

        let mut senders = Vec::new();
        let mut receivers = Vec::new();

        // Generate unique senders and receivers
        for _ in 0..num_senders {
            senders.push(generate_test_account());
        }

        for _ in 0..num_receivers {
            receivers.push(generate_test_account());
        }

        let mut transactions = Vec::new();

        // Create transactions between senders and receivers
        for (i, sender) in senders.iter().enumerate() {
            let receiver = receivers[i].clone();
            let transaction = create_signed_p2p_transaction(sender.clone(), vec![receiver.clone()]);
            transactions.extend(transaction);
        }

        let dependency_graph = DependencyGraph::create_dependency_graph(&transactions);
        assert_eq!(dependency_graph.size(), num_senders);

        let adjacency_list = dependency_graph.get_adjacency_list();
        let reverse_adjacency_list = dependency_graph.get_reverse_adjacency_list();
        for (_, dependencies) in adjacency_list.iter() {
            assert_eq!(dependencies.len(), 0);
        }
        for (_, reverse_dependencies) in reverse_adjacency_list.iter() {
            assert_eq!(reverse_dependencies.len(), 0);
        }
    }

    #[test]
    fn test_chained_txns() {
        let mut accounts = Vec::new();
        let num_txns = 10;
        for _ in 0..num_txns {
            accounts.push(generate_test_account());
        }
        let mut transactions = Vec::new();

        for i in 0..num_txns {
            let sender = accounts[i].clone();
            let receiver = accounts[(i + 1) % num_txns].clone();
            let transaction = create_signed_p2p_transaction(sender, vec![receiver]);
            transactions.extend(transaction);
        }
        let dependency_graph = DependencyGraph::create_dependency_graph(&transactions);
        assert_eq!(dependency_graph.size(), num_txns);
        let adjacency_list = dependency_graph.get_adjacency_list();
        let reverse_adjacency_list = dependency_graph.get_reverse_adjacency_list();
        assert_eq!(adjacency_list.len(), num_txns);
        assert_eq!(reverse_adjacency_list.len(), num_txns);

        fn assert_dependencies<'a, I>(dependencies: I, num_txns: usize)
        where
            I: Iterator<Item = (&'a Node<'a>, &'a HashSet<Node<'a>>)>,
        {
            for (node, dependencies) in dependencies {
                assert_eq!(dependencies.len(), 2);
                let index = node.index();
                let prev_index = if index == 0 { num_txns - 1 } else { index - 1 };
                let mut expected_indices: HashSet<usize> = vec![(index + 1) % num_txns, prev_index]
                    .into_iter()
                    .collect();
                for dependency in dependencies {
                    expected_indices.remove(&dependency.index());
                }
                assert_eq!(expected_indices.len(), 0);
            }
        }

        assert_dependencies(adjacency_list.iter(), num_txns);
        assert_dependencies(reverse_adjacency_list.iter(), num_txns);
    }

    #[test]
    fn test_no_dependency_txns() {
        // Create a set of transactions without any dependencies
        let num_txns = 10;
        let transactions = (0..num_txns)
            .flat_map(|_| create_no_dependency_transaction(1))
            .collect::<Vec<_>>();

        let dependency_graph = DependencyGraph::create_dependency_graph(&transactions);
        assert_eq!(dependency_graph.size(), num_txns);

        let adjacency_list = dependency_graph.get_adjacency_list();
        let reverse_adjacency_list = dependency_graph.get_reverse_adjacency_list();

        // Ensure that the adjacency list is empty for all transactions
        for (_, dependencies) in adjacency_list.iter() {
            assert!(dependencies.is_empty());
        }

        // Ensure that the reverse adjacency list is empty for all transactions
        for (_, reverse_dependencies) in reverse_adjacency_list.iter() {
            assert!(reverse_dependencies.is_empty());
        }
    }
}