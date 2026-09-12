use hbb_common::{bail, tokio, ResultType};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
};

type TxQueryOnlines = SyncSender<Vec<String>>;
lazy_static::lazy_static! {
    static ref TX_QUERY_ONLINES: Arc<Mutex<Option<TxQueryOnlines>>> = Default::default();
}

#[inline]
pub fn start_flutter_async_runner() {
    std::thread::spawn(start_flutter_async_runner_);
}

#[allow(dead_code)]
pub fn stop_flutter_async_runner() {
    let _ = TX_QUERY_ONLINES.lock().unwrap().take();
}

#[tokio::main(flavor = "current_thread")]
async fn start_flutter_async_runner_() {
    // Only one task is allowed to run at the same time.
    let (tx_onlines, rx_onlines) = sync_channel::<Vec<String>>(1);
    TX_QUERY_ONLINES.lock().unwrap().replace(tx_onlines);

    loop {
        match rx_onlines.recv() {
            Ok(ids) => {
                crate::client::peer_online::query_online_states(ids, handle_query_onlines).await
            }
            _ => {
                // unreachable!
                break;
            }
        }
    }
}

pub fn query_onlines(ids: Vec<String>) -> ResultType<()> {
    if let Some(tx) = TX_QUERY_ONLINES.lock().unwrap().as_ref() {
        // Ignore if the channel is full.
        let _ = tx.try_send(ids)?;
    } else {
        bail!("No tx_query_onlines");
    }
    Ok(())
}

fn handle_query_onlines(onlines: Vec<String>, offlines: Vec<String>) {
    let data = HashMap::from([
        ("name", "callback_query_onlines".to_owned()),
        ("onlines", onlines.join(",")),
        ("offlines", offlines.join(",")),
    ]);
    let _res = super::push_global_event(
        super::APP_TYPE_MAIN,
        serde_json::ser::to_string(&data).unwrap_or("".to_owned()),
    );
}
