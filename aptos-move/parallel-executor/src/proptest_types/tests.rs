// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use crate::{
    errors::Error,
    executor::ParallelTransactionExecutor,
    proptest_types::{
        baseline::ExpectedOutput,
        types::{
            FromU128, KeyType, PathKind, Task, Transaction, TransactionGen, TransactionGenParams,
            ValueType,
        },
    },
};
use aptos_aggregator::delta_change_set::serialize;
use claims::assert_ok;
use num_cpus;
use proptest::{
    collection::vec,
    prelude::*,
    sample::Index,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
use std::{fmt::Debug, hash::Hash};

const DEFAULT_STORAGE_VAL: u128 = 10000000;

fn run_transactions<K, V>(
    key_universe: &[K],
    transaction_gens: Vec<TransactionGen<V>>,
    abort_transactions: Vec<Index>,
    skip_rest_transactions: Vec<Index>,
    num_repeat: usize,
    module_access: (bool, bool),
) where
    K: Hash + Clone + Debug + Eq + Send + Sync + Ord + 'static,
    V: Clone + Eq + Send + Sync + Arbitrary + FromU128 + 'static,
    Vec<u8>: From<V>,
{
    let mut transactions: Vec<_> = transaction_gens
        .into_iter()
        .map(|txn_gen| txn_gen.materialize(key_universe, module_access))
        .collect();

    let length = transactions.len();
    for i in abort_transactions {
        *transactions.get_mut(i.index(length)).unwrap() = Transaction::Abort;
    }
    for i in skip_rest_transactions {
        *transactions.get_mut(i.index(length)).unwrap() = Transaction::SkipRest;
    }

    let cpu_count = num_cpus::get();
    for num_cpus in [cpu_count / 4, cpu_count / 2, cpu_count] {
        if num_cpus <= 1 {
            continue;
        }
        for _ in 0..num_repeat {
            let output = ParallelTransactionExecutor::<
                Transaction<KeyType<K>, ValueType<V>>,
                Task<KeyType<K>, ValueType<V>>,
            >::new(num_cpus)
            .execute_transactions_parallel(DEFAULT_STORAGE_VAL, &transactions)
            .map(|(res, _)| res);

            if module_access.0 && module_access.1 {
                assert_eq!(output.unwrap_err(), Error::ModulePathReadWrite);
                continue;
            }

            let baseline = ExpectedOutput::generate_baseline(&transactions, None, None);

            baseline.assert_output(&output, false);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn no_early_termination(
        universe in vec(any::<[u8; 32]>(), 100),
        transaction_gen in vec(any::<TransactionGen<[u8;32]>>(), 5000).no_shrink(),
        abort_transactions in vec(any::<Index>(), 0),
        skip_rest_transactions in vec(any::<Index>(), 0),
    ) {
        run_transactions(&universe, transaction_gen, abort_transactions, skip_rest_transactions, 1, (false, false));
    }

    #[test]
    fn abort_only(
        universe in vec(any::<[u8; 32]>(), 100),
        transaction_gen in vec(any::<TransactionGen<[u8;32]>>(), 5000).no_shrink(),
        abort_transactions in vec(any::<Index>(), 5),
        skip_rest_transactions in vec(any::<Index>(), 0),
    ) {
        run_transactions(&universe, transaction_gen, abort_transactions, skip_rest_transactions, 1, (false, false));
    }

    #[test]
    fn skip_rest_only(
        universe in vec(any::<[u8; 32]>(), 100),
        transaction_gen in vec(any::<TransactionGen<[u8;32]>>(), 5000).no_shrink(),
        abort_transactions in vec(any::<Index>(), 0),
        skip_rest_transactions in vec(any::<Index>(), 5),
    ) {
        run_transactions(&universe, transaction_gen, abort_transactions, skip_rest_transactions, 1, (false, false));
    }

    #[test]
    fn mixed_transactions(
        universe in vec(any::<[u8; 32]>(), 100),
        transaction_gen in vec(any::<TransactionGen<[u8;32]>>(), 5000).no_shrink(),
        abort_transactions in vec(any::<Index>(), 5),
        skip_rest_transactions in vec(any::<Index>(), 5),
    ) {
        run_transactions(&universe, transaction_gen, abort_transactions, skip_rest_transactions, 1, (false, false));
    }

    #[test]
    fn dynamic_read_writes_mixed(
        universe in vec(any::<[u8; 32]>(), 100),
        transaction_gen in vec(any_with::<TransactionGen<[u8;32]>>(TransactionGenParams::new_dynamic()), 3000).no_shrink(),
        abort_transactions in vec(any::<Index>(), 3),
        skip_rest_transactions in vec(any::<Index>(), 3),
    ) {
        run_transactions(&universe, transaction_gen, abort_transactions, skip_rest_transactions, 1, (false, false));
    }
}

#[test]
fn dynamic_read_writes() {
    let mut runner = TestRunner::default();

    let universe = vec(any::<[u8; 32]>(), 100)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();
    let transaction_gen = vec(
        any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
        3000,
    )
    .new_tree(&mut runner)
    .expect("creating a new value should succeed")
    .current();

    run_transactions(
        &universe,
        transaction_gen,
        vec![],
        vec![],
        100,
        (false, false),
    );
}

#[test]
fn deltas_writes_mixed() {
    let mut runner = TestRunner::default();
    let num_txns = 1000;

    let universe = vec(any::<[u8; 32]>(), 50)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();

    let cpu_count = num_cpus::get();
    for num_cpus in [cpu_count / 2, cpu_count] {
        for i in 0..5 {
            let transaction_gen = vec(
                any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
                num_txns,
            )
            .new_tree(&mut runner)
            .expect("creating a new value should succeed")
            .current();

            let limit = if i == 0 { 100 } else { u128::MAX };
            let transactions: Vec<_> = transaction_gen
                .into_iter()
                .map(|txn_gen| txn_gen.materialize_with_deltas(&universe, 15, true, limit))
                .collect();

            for _ in 0..8 {
                let output = ParallelTransactionExecutor::<
                    Transaction<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
                    Task<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
                >::new(num_cpus)
                .execute_transactions_parallel(DEFAULT_STORAGE_VAL, &transactions)
                .map(|(res, _)| res);

                let baseline = ExpectedOutput::generate_baseline(&transactions, None, None);

                baseline.assert_output(&output, false);
            }
        }
    }
}

#[test]
fn deltas_writes_mixed_sequential() {
    let mut runner = TestRunner::default();
    let num_txns = 1000;

    for _ in 0..5 {
        let universe = vec(any::<[u8; 32]>(), 50)
            .new_tree(&mut runner)
            .expect("creating a new value should succeed")
            .current();
        let transaction_gen = vec(
            any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
            num_txns,
        )
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();

        let transactions: Vec<_> = transaction_gen
            .into_iter()
            .map(|txn_gen| txn_gen.materialize_with_deltas(&universe, 15, true, u128::MAX))
            .collect();

        // Do not allow deletes as that would lead to delta application errors (sequential
        // materializes and currently errors not supported even in testing).
        let storage_aggregator_val = 100001;
        let sequential_output = ParallelTransactionExecutor::<
            Transaction<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
            Task<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
        >::new(2) // Does not matter, we use sequential.
        .execute_transactions_sequential(storage_aggregator_val, &transactions);

        let baseline =
            ExpectedOutput::generate_baseline(&transactions, None, Some(storage_aggregator_val));
        baseline.assert_output(&sequential_output, true);
    }
}

#[test]
fn deltas_resolver() {
    let mut runner = TestRunner::default();
    let num_txns = 1000;

    let universe = vec(any::<[u8; 32]>(), 50)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();
    let transaction_gen = vec(
        any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
        num_txns,
    )
    .new_tree(&mut runner)
    .expect("creating a new value should succeed")
    .current();

    // Do not allow deletes as that would panic in resolver.
    let transactions: Vec<_> = transaction_gen
        .into_iter()
        .map(|txn_gen| txn_gen.materialize_with_deltas(&universe, 15, false, u128::MAX))
        .collect();

    // Should not be possible to overflow or underflow, as each delta is at
    // most 100 in the tests.
    let storage_aggregator_val = 100001;

    let cpu_count = num_cpus::get();
    for num_cpus in [cpu_count / 2, cpu_count] {
        for _ in 0..15 {
            let output = ParallelTransactionExecutor::<
                Transaction<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
                Task<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
            >::new(num_cpus)
            .execute_transactions_parallel(DEFAULT_STORAGE_VAL, &transactions);

            let (output, delta_resolver) = output.unwrap();
            let resolved = delta_resolver.resolve(
                (15..50)
                    .map(|i| {
                        (
                            KeyType(universe[i], PathKind::Data),
                            Ok(Some(serialize(&storage_aggregator_val))),
                        )
                    })
                    .collect(),
                num_txns,
            );

            let baseline = ExpectedOutput::generate_baseline(
                &transactions,
                Some(resolved),
                Some(storage_aggregator_val),
            );
            baseline.assert_output(&Ok(output), false);
        }
    }
}

#[test]
fn dynamic_read_writes_contended() {
    let mut runner = TestRunner::default();

    let universe = vec(any::<[u8; 32]>(), 10)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();

    let transaction_gen = vec(
        any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
        1000,
    )
    .new_tree(&mut runner)
    .expect("creating a new value should succeed")
    .current();

    run_transactions(
        &universe,
        transaction_gen,
        vec![],
        vec![],
        100,
        (false, false),
    );
}

#[test]
fn module_publishing_fallback() {
    let mut runner = TestRunner::default();

    let universe = vec(any::<[u8; 32]>(), 100)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();
    let transaction_gen = vec(
        any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
        3000,
    )
    .new_tree(&mut runner)
    .expect("creating a new value should succeed")
    .current();

    run_transactions(
        &universe,
        transaction_gen.clone(),
        vec![],
        vec![],
        2,
        (false, true),
    );
    run_transactions(
        &universe,
        transaction_gen.clone(),
        vec![],
        vec![],
        2,
        (false, true),
    );
    run_transactions(&universe, transaction_gen, vec![], vec![], 2, (true, true));
}

fn publishing_fixed_params() {
    let mut runner = TestRunner::default();
    let num_txns = 300;

    let universe = vec(any::<[u8; 32]>(), 50)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();
    let transaction_gen = vec(
        any_with::<TransactionGen<[u8; 32]>>(TransactionGenParams::new_dynamic()),
        num_txns,
    )
    .new_tree(&mut runner)
    .expect("creating a new value should succeed")
    .current();
    let indices = vec(any::<Index>(), 4)
        .new_tree(&mut runner)
        .expect("creating a new value should succeed")
        .current();

    // First 12 keys are normal paths, next 14 are module reads, then writes.
    let mut transactions: Vec<_> = transaction_gen
        .into_iter()
        .map(|txn_gen| txn_gen.materialize_disjoint_module_rw(&universe[0..40], 12, 26))
        .collect();

    // Adjust the writes of txn indices[0] to contain module write to key 42.
    let w_index = indices[0].index(num_txns);
    *transactions.get_mut(w_index).unwrap() = match transactions.get_mut(w_index).unwrap() {
        Transaction::Write {
            incarnation,
            reads,
            writes_and_deltas,
        } => {
            let mut new_writes_and_deltas = vec![];
            for (incarnation_writes, incarnation_deltas) in writes_and_deltas {
                assert!(!incarnation_writes.is_empty());
                let val = incarnation_writes[0].1.clone();
                let insert_idx = indices[1].index(incarnation_writes.len());
                incarnation_writes
                    .insert(insert_idx, (KeyType(universe[42], PathKind::Module), val));
                new_writes_and_deltas
                    .push((incarnation_writes.clone(), incarnation_deltas.clone()));
            }

            Transaction::Write {
                incarnation: incarnation.clone(),
                reads: reads.clone(),
                writes_and_deltas: new_writes_and_deltas,
            }
        }
        _ => {
            unreachable!();
        }
    };

    // Confirm still no intersection
    let output = ParallelTransactionExecutor::<
        Transaction<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
        Task<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
    >::new(num_cpus::get())
    .execute_transactions_parallel(DEFAULT_STORAGE_VAL, &transactions);
    assert_ok!(output);

    // Adjust the reads of txn indices[2] to contain module read to key 42.
    let r_index = indices[2].index(num_txns);
    *transactions.get_mut(r_index).unwrap() = match transactions.get_mut(r_index).unwrap() {
        Transaction::Write {
            incarnation,
            reads,
            writes_and_deltas,
        } => {
            let mut new_reads = vec![];
            for incarnation_reads in reads {
                assert!(!incarnation_reads.is_empty());
                let insert_idx = indices[3].index(incarnation_reads.len());
                incarnation_reads.insert(insert_idx, KeyType(universe[42], PathKind::Module));
                new_reads.push(incarnation_reads.clone());
            }

            Transaction::Write {
                incarnation: incarnation.clone(),
                reads: new_reads,
                writes_and_deltas: writes_and_deltas.clone(),
            }
        }
        _ => {
            unreachable!();
        }
    };

    for _ in 0..200 {
        let output = ParallelTransactionExecutor::<
            Transaction<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
            Task<KeyType<[u8; 32]>, ValueType<[u8; 32]>>,
        >::new(num_cpus::get())
        .execute_transactions_parallel(DEFAULT_STORAGE_VAL, &transactions)
        .map(|(res, _)| res);

        assert_eq!(output.unwrap_err(), Error::ModulePathReadWrite);
    }
}

#[test]
// Test a single transaction intersection interleaves with a lot of dependencies and
// not overlapping module r/w keys.
fn module_publishing_races() {
    for _ in 0..10 {
        publishing_fixed_params();
    }
}
