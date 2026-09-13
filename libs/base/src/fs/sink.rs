//! Where a file transfer's messages go.
//!
//! A file transfer writes its blocks straight at the peer, and the blocks of one job must
//! arrive in the order they were produced or the file on the other side is wrong. That is
//! the whole reason this is a trait rather than a channel handed in: whatever implements
//! it has to promise order, and each implementation says how it keeps that promise.
//!
//! Two implementations exist. A connection that owns its socket writes through it and is
//! ordered because there is one writer. A connection whose writing half belongs to a task
//! queues instead, and is ordered because the queue is a queue.

use crate::message_proto::Message;
use hbb_common::ResultType;

#[async_trait::async_trait]
pub trait MsgSink: Send {
    /// Sends one message. Messages sent through the same sink reach the peer in the order
    /// they were sent, which file transfer depends on.
    ///
    /// By value, so that a queueing implementation can take the message rather than copy
    /// it: a file block is tens of kilobytes and there is one per read.
    async fn send_msg(&mut self, msg: Message) -> ResultType<()>;
}

#[async_trait::async_trait]
impl MsgSink for hbb_common::Stream {
    async fn send_msg(&mut self, msg: Message) -> ResultType<()> {
        self.send(&msg).await
    }
}
