// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use crate::{
    quorum_store::{
        batch_aggregator::BatchAggregator,
        batch_reader::BatchReaderCommand,
        batch_store::{BatchStoreCommand, PersistRequest},
        counters,
        proof_builder::ProofBuilderCommand,
        types::Fragment,
    },
    round_manager::VerifiedEvent,
};
use aptos_channels::aptos_channel;
use aptos_logger::debug;
use aptos_types::PeerId;
use futures::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;

pub(crate) struct NetworkListener {
    // TODO: reconsider which fields are needed.
    epoch: u64,
    network_msg_rx: aptos_channel::Receiver<PeerId, VerifiedEvent>,
    batch_aggregators: HashMap<PeerId, BatchAggregator>,
    batch_store_tx: Sender<BatchStoreCommand>,
    batch_reader_tx: Sender<BatchReaderCommand>,
    proof_builder_tx: Sender<ProofBuilderCommand>,
    max_batch_bytes: usize,
}

impl NetworkListener {
    pub(crate) fn new(
        epoch: u64,
        network_msg_rx: aptos_channel::Receiver<PeerId, VerifiedEvent>,
        batch_store_tx: Sender<BatchStoreCommand>,
        batch_reader_tx: Sender<BatchReaderCommand>,
        proof_builder_tx: Sender<ProofBuilderCommand>,
        max_batch_bytes: usize,
    ) -> Self {
        Self {
            epoch,
            network_msg_rx,
            batch_aggregators: HashMap::new(),
            batch_store_tx,
            batch_reader_tx,
            proof_builder_tx,
            max_batch_bytes,
        }
    }

    async fn handle_fragment(&mut self, fragment: Fragment) {
        let source = fragment.source();
        let entry = self
            .batch_aggregators
            .entry(source)
            .or_insert(BatchAggregator::new(self.max_batch_bytes));
        if let Some(expiration) = fragment.maybe_expiration() {
            counters::DELIVERED_END_BATCH_COUNT.inc();
            // end batch message
            debug!(
                "QS: got end batch message from {:?} batch_id {}, fragment_id {}",
                source,
                fragment.batch_id(),
                fragment.fragment_id(),
            );
            if expiration.epoch() == self.epoch {
                match entry.end_batch(
                    fragment.batch_id(),
                    fragment.fragment_id(),
                    fragment.into_transactions(),
                ) {
                    Ok((num_bytes, payload, digest)) => {

                        if payload.iter().all(|txn| txn.only_check_signature().is_ok()) {
                            let persist_cmd = BatchStoreCommand::Persist(PersistRequest::new(
                                source, payload, digest, num_bytes, expiration,
                            ));

                            self.batch_store_tx
                                .send(persist_cmd)
                                .await
                                .expect("BatchStore receiver not available");
                        }
                    }
                    Err(e) => {
                        debug!("Could not append batch from {:?}, error {:?}", source, e);
                    }
                }
            }
            // Malformed request with an inconsistent expiry epoch.
            else {
                debug!(
                    "QS: got end batch message epoch {} {}",
                    expiration.epoch(),
                    self.epoch
                );
            }
        } else {
            debug!("QS: fragment no expiration");
            // debug!(
            //     "QS: got append_batch message from {:?} batch_id {}, fragment_id {}",
            //     source,
            //     fragment.fragment_info.batch_id(),
            //     fragment.fragment_info.fragment_id()
            // );
            if let Err(e) = entry.append_transactions(
                fragment.batch_id(),
                fragment.fragment_id(),
                fragment.into_transactions(),
            ) {
                debug!("Could not append batch from {:?}, error {:?}", source, e);
            }
        }
    }

    pub async fn start(mut self) {
        debug!("QS: starting networking");
        //batch fragment -> batch_aggregator, persist it, and prapre signedDigests
        //Keep in memory caching in side the DB wrapper.
        //chack id -> self, call PoQSB.
        while let Some(msg) = self.network_msg_rx.next().await {
            // debug!("QS: network_listener msg {:?}", msg);
            match msg {
                VerifiedEvent::Shutdown(ack_tx) => {
                    debug!("QS: shutdown network listener received");
                    ack_tx
                        .send(())
                        .expect("Failed to send shutdown ack to QuorumStore");
                    break;
                }
                VerifiedEvent::SignedDigestMsg(signed_digest) => {
                    // debug!("QS: got SignedDigest from network");
                    let cmd = ProofBuilderCommand::AppendSignature(*signed_digest);
                    self.proof_builder_tx
                        .send(cmd)
                        .await
                        .expect("Could not send signed_digest to proof_builder");
                }

                VerifiedEvent::FragmentMsg(fragment) => {
                    counters::DELIVERED_FRAGMENTS_COUNT.inc();
                    self.handle_fragment(*fragment).await;
                }

                VerifiedEvent::BatchRequestMsg(request) => {
                    counters::RECEIVED_BATCH_REQUEST_COUNT.inc();
                    debug!(
                        "QS: batch request from {:?} digest {}",
                        request.source(),
                        request.digest()
                    );
                    let cmd =
                        BatchReaderCommand::GetBatchForPeer(request.digest(), request.source());
                    self.batch_reader_tx
                        .send(cmd)
                        .await
                        .expect("could not push Batch batch_reader");
                }

                VerifiedEvent::UnverifiedBatchMsg(batch) => {
                    counters::RECEIVED_BATCH_COUNT.inc();
                    debug!(
                        "QS: batch response from {:?} digest {}",
                        batch.source(),
                        batch.digest()
                    );
                    let cmd =
                        BatchReaderCommand::BatchResponse(batch.digest(), batch.into_payload());
                    self.batch_reader_tx
                        .send(cmd)
                        .await
                        .expect("could not push Batch batch_reader");
                }

                _ => {
                    unreachable!()
                }
            };
        }
    }
}
