//! Exception types exposed by the `rurex` module.

use pyo3::create_exception;

// Error: raised for pattern compilation errors; subclasses `Exception`,
// exactly like `re.error` (ADR-0004).
create_exception!(rurex, Error, pyo3::exceptions::PyException);
