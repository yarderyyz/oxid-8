use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

pub fn spawn_timers(dt: Arc<AtomicU8>, st: Arc<AtomicU8>) -> mpsc::Receiver<bool> {
    let (tx, rx) = mpsc::channel::<bool>();
    thread::spawn(move || {
        let mut sounding = false;
        loop {
            let _ = dt.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                (v > 0).then(|| v - 1)
            });
            let _ = st.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                (v > 0).then(|| v - 1)
            });

            let st_now = st.load(Ordering::Acquire);
            if st_now > 0 && !sounding {
                let _ = tx.send(true);
                sounding = true;
            } else if st_now == 0 && sounding {
                let _ = tx.send(false);
                sounding = false;
            }

            thread::sleep(Duration::from_nanos(16_666_667)); // ~60 Hz
        }
    });
    rx
}
