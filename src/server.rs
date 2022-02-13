use std::sync::{Arc, Mutex};
use std::thread;

use tiny_http::{Method, Response, Server};

/// A thread-safe growable array.
#[derive(Clone)]
pub struct MetricsServer(Arc<Mutex<Vec<u8>>>);

impl Default for MetricsServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsServer {
    /// Creates a new empty `MetricsServer`.
    ///
    /// This will create a mutex protected empty Vector. It will not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use metrics_server::MetricsServer;
    ///
    /// let server = MetricsServer::new();
    /// ```
    pub fn new() -> Self {
        MetricsServer(Arc::new(Mutex::new(Vec::new())))
    }

    /// Safely updates the data in a `MetricsServer` and returns the number of
    /// bytes written.
    ///
    /// This function is thread safe and protected by a mutex. It is safe
    /// to call concurrently from multiple threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use metrics_server::MetricsServer;
    ///
    /// let server = MetricsServer::new();
    /// let bytes = server.update(Vec::from([1, 2, 3, 4]));
    /// assert_eq!(bytes, 4);
    /// ```
    pub fn update(&self, data: Vec<u8>) -> usize {
        let mut buf = self.0.lock().unwrap();
        *buf = data;
        buf.as_slice().len()
    }

    /// Starts a simple HTTP server on a new thread at the given address and expose the stored metrics.
    /// This server is intended to only be queried synchronously as it blocks upon receiving
    /// each request.
    ///
    /// # Examples
    ///
    /// ```
    /// use metrics_server::MetricsServer;
    ///
    /// let server = MetricsServer::new();
    /// server.serve("localhost:8001");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if given an invalid address.
    pub fn serve(&self, addr: &str) {
        // Create a new HTTP server and bind to the given address.
        let server = Server::http(addr).unwrap();

        // Invoking clone on Arc produces a new Arc instance, which points to the
        // same allocation on the heap as the source Arc, while increasing a reference count.
        let buf = Arc::clone(&self.0);

        // Handle requests in a new thread so we can process in the background.
        thread::spawn({
            move || {
                loop {
                    // Blocks until the next request is received.
                    let req = match server.recv() {
                        Ok(req) => req,
                        Err(e) => {
                            eprintln!("error: {}", e);
                            continue;
                        }
                    };

                    // Only respond to GET requests(?).
                    if req.method() != &Method::Get {
                        let res = Response::empty(405);
                        if let Err(e) = req.respond(res) {
                            eprintln!("{}", e);
                        };
                        continue;
                    }

                    // TODO: this is naive. Fix it(?)
                    // Only serve the /metrics path.
                    if req.url() != "/metrics" {
                        let res = Response::empty(404);
                        if let Err(e) = req.respond(res) {
                            eprintln!("{}", e);
                        };
                        continue;
                    }

                    // Write the metrics to the response buffer.
                    let metrics = buf.lock().unwrap();
                    let res = Response::from_data(metrics.as_slice());
                    if let Err(e) = req.respond(res) {
                        eprintln!("{}", e);
                    };
                }
            }
        });
    }
}
