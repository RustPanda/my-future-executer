use std::{
    cmp,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, RawWaker, Wake, Waker},
    thread,
    time::Duration,
};

use crossbeam_channel::{unbounded, Sender};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct ThreadPool {
    task_sender: Sender<Arc<Task>>,
}

struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: Sender<Arc<Task>>,
}
impl Wake for Task {
    fn wake_by_ref(self: &Arc<Self>) {
        self.task_sender.send(self.clone()).unwrap();
    }

    fn wake(self: Arc<Self>) {
        self.task_sender.send(self.clone()).unwrap();
    }
}

impl ThreadPool {
    fn new() -> Self {
        let len_pool = cmp::max(1, num_cpus::get());

        let (task_sender, ready_queue) = unbounded::<Arc<Task>>();

        {
            for _i in 0..len_pool {
                let ready_queue_clone = ready_queue.clone();

                thread::spawn(move || {
                    while let Ok(task) = ready_queue_clone.recv() {
                        let mut future_slot = task.future.lock().unwrap();
                        if let Some(mut future) = future_slot.take() {
                            let raw_waker = RawWaker::from(task.clone());
                            let waker = unsafe { Waker::from_raw(raw_waker) };
                            let context = &mut Context::from_waker(&waker);

                            if let Poll::Pending = future.as_mut().poll(context) {
                                *future_slot = Some(future);
                            }
                        }
                    }
                });
            }
        }
        Self { task_sender }
    }

    fn spawn(&mut self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);

        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        self.task_sender.send(task).expect("too many tasks queued");
    }
}

fn main() {
    println!("Hello, world!");

    let mut pool = ThreadPool::new();

    for i in 0..1000 {
        pool.spawn(async move {
            async { println!("{} {:?}", i, thread::current().id()) }.await;
            let a = async { String::from("Hello") }.await;
            dbg!(a);
        });
    }

    thread::sleep(Duration::from_secs(10));
}
