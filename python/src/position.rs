use nanobook::portfolio::Position;
use pyo3::prelude::*;

#[pyclass(name = "Position", from_py_object)]
#[derive(Clone)]
pub struct PyPosition {
    pub inner: Position,
}

#[pymethods]
impl PyPosition {
    #[getter]
    fn symbol(&self) -> String {
        self.inner.symbol.to_string()
    }

    /// Position quantity as a fractional share count (e.g. `0.001`).
    ///
    /// Backed by a fixed-point micro-share integer (1 share = 1_000_000 units);
    /// converting through `f64` loses precision beyond that resolution. Use
    /// `quantity_micro` for an exact, lossless round trip.
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity.to_f64()
    }

    /// Position quantity in raw micro-share units (1 share = 1_000_000 units).
    /// Exact and lossless — the native representation.
    #[getter]
    fn quantity_micro(&self) -> i64 {
        self.inner.quantity.raw()
    }

    /// Position quantity truncated to whole shares, dropping any fractional
    /// remainder. Only useful for callers that genuinely want whole shares
    /// (e.g. legacy reporting); use `quantity` or `quantity_micro` otherwise.
    #[getter]
    fn quantity_whole(&self) -> i64 {
        self.inner.quantity.whole()
    }

    #[getter]
    fn avg_entry_price(&self) -> i64 {
        self.inner.avg_entry_price
    }

    #[getter]
    fn total_cost(&self) -> i64 {
        self.inner.total_cost
    }

    #[getter]
    fn realized_pnl(&self) -> i64 {
        self.inner.realized_pnl
    }

    fn unrealized_pnl(&self, price: i64) -> i64 {
        self.inner.unrealized_pnl(price)
    }

    fn __repr__(&self) -> String {
        format!(
            "Position(symbol={}, qty={}, avg_price={}, realized_pnl={})",
            self.inner.symbol,
            self.inner.quantity.to_f64(),
            self.inner.avg_entry_price,
            self.inner.realized_pnl
        )
    }
}
