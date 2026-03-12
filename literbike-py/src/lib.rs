#![allow(unused)]

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Literbike QUIC Transport Client for Python
///
/// Provides low-latency QUIC transport for external agents
#[pyclass]
struct QuicClient {
    runtime: Arc<Runtime>,
}

#[pymethods]
impl QuicClient {
    #[new]
    fn new() -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {}", e))
        })?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    /// Get Literbike version info
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Test basic connectivity
    fn ping(&self) -> PyResult<String> {
        Ok("pong".to_string())
    }

    /// Get network interface information
    fn get_interfaces(&self) -> PyResult<Vec<PyObject>> {
        Python::with_gil(|py| {
            let interfaces = vec![{
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("name", "lo")?;
                dict.set_item("ip", "127.0.0.1")?;
                dict.set_item("flags", "UP,LOOPBACK")?;
                dict.into()
            }];
            Ok(interfaces)
        })
    }

    /// Check if a host is reachable
    fn probe_host(&self, host: &str, port: u16, timeout_ms: u64) -> PyResult<bool> {
        // Placeholder - will be implemented with actual syscall_net
        Ok(true)
    }
}

/// Network utility functions
#[pyfunction]
fn get_hostname() -> PyResult<String> {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get hostname: {}", e)))
}

#[pyfunction]
fn get_pid() -> u32 {
    std::process::id()
}

/// Python module definition
#[pymodule]
fn literbike_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<QuicClient>()?;
    m.add_function(wrap_pyfunction!(get_hostname, m)?)?;
    m.add_function(wrap_pyfunction!(get_pid, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
