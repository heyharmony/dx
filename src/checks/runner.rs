use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use super::StartupCheck;
use super::context::CheckContext;
use super::types::Suggestion;

pub struct CheckRunner {
    checks: Vec<Arc<dyn StartupCheck>>,
}

impl Default for CheckRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    #[must_use]
    pub fn register<C: StartupCheck + 'static>(mut self, check: C) -> Self {
        self.checks.push(Arc::new(check));
        self
    }

    /// Run all checks in parallel threads and collect the full list.
    #[must_use]
    pub fn run_all(&self, ctx: &CheckContext) -> Vec<Suggestion> {
        let mut handles = Vec::new();
        for check in &self.checks {
            let c = Arc::clone(check);
            let ctx = ctx.clone();
            let handle = thread::spawn(move || c.run(&ctx));
            handles.push(handle);
        }
        let mut all = Vec::new();
        for h in handles {
            if let Ok(mut v) = h.join() {
                all.append(&mut v);
            }
        }
        all
    }

    /// Run all checks in parallel and stream suggestions via channel as they complete.
    #[must_use]
    pub fn run_stream(&self, ctx: &CheckContext) -> Receiver<Suggestion> {
        let (tx, rx) = mpsc::channel::<Suggestion>();
        let mut handles = Vec::new();
        for check in &self.checks {
            let tx = tx.clone();
            let c = Arc::clone(check);
            let ctx = ctx.clone();
            let handle = thread::spawn(move || {
                let results = c.run(&ctx);
                for s in results {
                    let _ = tx.send(s);
                }
            });
            handles.push(handle);
        }
        // Drop original sender so channel closes when all threads finish
        drop(tx);
        // Detach a waiter thread to join workers and then return
        thread::spawn(move || {
            for h in handles {
                let _ = h.join();
            }
        });
        rx
    }
}
