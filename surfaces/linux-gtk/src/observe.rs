use arut_watch::Subscription;
use gtk::glib;
use std::{future::Future, sync::Arc};

/// Aborts observers and stream followers when their component leaves the UI.
#[derive(Default)]
pub struct Tasks(Vec<glib::JoinHandle<()>>);

impl Tasks {
    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        self.0.push(glib::spawn_future_local(future));
    }

    pub fn watch<M: Clone + 'static>(
        &mut self,
        changes: Arc<Subscription<u64>>,
        sender: relm4::Sender<M>,
        message: M,
    ) {
        self.spawn(async move {
            while changes.changed().await.is_some() {
                if sender.send(message.clone()).is_err() {
                    break;
                }
            }
        });
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        for task in self.0.drain(..) {
            task.abort();
        }
    }
}
